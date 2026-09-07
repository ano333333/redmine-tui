//! Uploads edited Remote Journal notes to Redmine.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, Dispatcher, IssueAction, IssueState, JournalAction, JournalEntry, JournalUploadFailure,
    JournalUploadFailureStage, RemoteJournalState, RemoteJournalUploadConflict,
};
use crate::vos::{JournalId, JournalKey, JournalNotesDiff};

pub type UploadRemoteJournalFuture = Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum RemoteJournalNotesDecision {
    AlreadyApplied,
    Upload {
        notes: String,
    },
    Conflict {
        before: String,
        after: String,
        server: String,
    },
}

fn compare_remote_journal_notes(
    diff: &JournalNotesDiff,
    server: &str,
) -> RemoteJournalNotesDecision {
    if server == diff.before {
        RemoteJournalNotesDecision::Upload {
            notes: diff.after.clone(),
        }
    } else if server == diff.after {
        RemoteJournalNotesDecision::AlreadyApplied
    } else {
        RemoteJournalNotesDecision::Conflict {
            before: diff.before.clone(),
            after: diff.after.clone(),
            server: server.to_string(),
        }
    }
}

/// 編集済みRemote Journalの保存を開始する。
pub fn upload_remote_journal<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: JournalId,
) -> Option<UploadRemoteJournalFuture>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let key = JournalKey::Remote(id);
    let (issue_id, diff) = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        let (issue_id, state, diff) = match store.get_journal_entry(key) {
            Some(JournalEntry::Remote {
                issue_id,
                state,
                notes_diff,
                ..
            }) => (*issue_id, state, notes_diff),
            _ => panic!("remote journal does not exist"),
        };
        if state != &RemoteJournalState::Edited {
            return None;
        }
        let diff = diff
            .clone()
            .expect("edited remote journal must have a notes diff");
        let (_, issue_state) = store
            .get_issue(issue_id)
            .expect("issue of the remote journal does not exist");
        if *issue_state == IssueState::Uploading {
            panic!("issue is already uploading");
        }
        if store.has_uploading_journal(issue_id) {
            panic!("another journal of the issue is already uploading");
        }
        (issue_id, diff)
    };
    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartUpload { key });

    Some(Box::pin(async move {
        let (issue, journals) = match client.get_issue(issue_id).await {
            Ok(result) => result,
            Err(error) => {
                return failed(
                    id,
                    JournalUploadFailureStage::RemoteFetch,
                    error.to_string(),
                );
            }
        };
        if issue.issue.id != issue_id {
            return failed(
                id,
                JournalUploadFailureStage::RemoteFetch,
                format!(
                    "fetched issue ID {} does not match expected {}",
                    issue.issue.id, issue_id
                ),
            );
        }
        let Some(server) = journals.into_iter().find(|journal| journal.id == id) else {
            return vec![
                IssueAction::RemoveJournalKey { issue_id, key }.into(),
                JournalAction::RemoveUploadingRemote { id, issue_id }.into(),
            ];
        };
        match compare_remote_journal_notes(&diff, &server.notes) {
            RemoteJournalNotesDecision::AlreadyApplied => vec![
                JournalAction::CompleteRemoteUploadFromFetch {
                    journal: server,
                    issue_id,
                }
                .into(),
            ],
            RemoteJournalNotesDecision::Upload { notes } => {
                if let Err(error) = client.update_journal_notes(id, &notes).await {
                    return failed(id, JournalUploadFailureStage::RemotePut, error.to_string());
                }
                vec![JournalAction::CompleteRemoteUpload { id, notes }.into()]
            }
            RemoteJournalNotesDecision::Conflict {
                before,
                after,
                server: _,
            } => vec![
                JournalAction::UploadConflictsDetected {
                    conflict: RemoteJournalUploadConflict {
                        id,
                        issue_id,
                        before,
                        after,
                        server,
                    },
                }
                .into(),
            ],
        }
    }))
}

fn failed(id: JournalId, stage: JournalUploadFailureStage, message: String) -> Vec<Action> {
    vec![
        JournalAction::FailUpload {
            key: JournalKey::Remote(id),
            failure: JournalUploadFailure { stage, message },
        }
        .into(),
    ]
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Journal, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{
        Action, Dispatcher, IssueAction, JournalAction, JournalUploadFailureStage,
    };
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId, JournalKey, JournalNotesDiff};

    use super::{RemoteJournalNotesDecision, compare_remote_journal_notes, upload_remote_journal};

    #[test]
    fn uploads_local_notes_when_the_server_is_unchanged() {
        let diff = JournalNotesDiff {
            before: "before".to_string(),
            after: "after".to_string(),
        };

        assert_eq!(
            compare_remote_journal_notes(&diff, "before"),
            RemoteJournalNotesDecision::Upload {
                notes: "after".to_string()
            }
        );
    }

    #[test]
    fn skips_upload_when_the_server_already_has_local_notes() {
        let diff = JournalNotesDiff {
            before: "before".to_string(),
            after: "after".to_string(),
        };

        assert_eq!(
            compare_remote_journal_notes(&diff, "after"),
            RemoteJournalNotesDecision::AlreadyApplied
        );
    }

    #[test]
    fn reports_all_three_values_when_both_sides_changed() {
        let diff = JournalNotesDiff {
            before: "before".to_string(),
            after: "local".to_string(),
        };

        assert_eq!(
            compare_remote_journal_notes(&diff, "server"),
            RemoteJournalNotesDecision::Conflict {
                before: "before".to_string(),
                after: "local".to_string(),
                server: "server".to_string(),
            }
        );
    }

    #[test]
    fn treats_empty_notes_as_regular_values() {
        let cleared = JournalNotesDiff {
            before: "before".to_string(),
            after: String::new(),
        };
        assert_eq!(
            compare_remote_journal_notes(&cleared, "before"),
            RemoteJournalNotesDecision::Upload {
                notes: String::new()
            }
        );

        let added = JournalNotesDiff {
            before: String::new(),
            after: "after".to_string(),
        };
        assert_eq!(
            compare_remote_journal_notes(&added, "after"),
            RemoteJournalNotesDecision::AlreadyApplied
        );
    }

    #[tokio::test]
    async fn starts_upload_synchronously_before_the_get_request() {
        let dispatcher = edited_dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![journal(1, "before")]));

        let future = upload_remote_journal(dispatcher.clone(), client.clone(), 1.into());

        assert!(future.is_some());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn upload_future_is_send() {
        fn assert_send<T: Send>(_: T) {}
        let future = upload_remote_journal(
            edited_dispatcher(),
            Arc::new(StubClient::succeeds(issue(42), vec![])),
            1.into(),
        )
        .unwrap();

        assert_send(future);
    }

    #[test]
    fn completion_actions_are_send() {
        fn assert_send<T: Send>() {}

        assert_send::<Action>();
    }

    #[tokio::test]
    async fn returns_fail_upload_when_get_fails() {
        let dispatcher = edited_dispatcher();
        let client = Arc::new(StubClient::fails_get());

        let actions = upload_remote_journal(dispatcher, client, 1.into())
            .unwrap()
            .await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { key: JournalKey::Remote(id), failure })]
                if id == &JournalId::new(1)
                    && failure.stage == JournalUploadFailureStage::RemoteFetch
                    && failure.message == "network error: offline")
        );
    }

    #[tokio::test]
    async fn returns_fail_upload_when_get_returns_another_issue() {
        let dispatcher = edited_dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(99), vec![journal(1, "before")]));

        let actions = upload_remote_journal(dispatcher, client, 1.into())
            .unwrap()
            .await;

        assert!(matches!(
            actions.as_slice(),
            [Action::Journal(JournalAction::FailUpload { failure, .. })]
                if failure.stage == JournalUploadFailureStage::RemoteFetch
                    && failure.message.contains("does not match expected")
        ));
    }

    #[tokio::test]
    async fn removes_a_missing_journal_reference_before_the_entry() {
        let dispatcher = edited_dispatcher_with_local();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![journal(2, "other")]));

        let actions = upload_remote_journal(dispatcher, client.clone(), 1.into())
            .unwrap()
            .await;

        let [
            Action::Issue(IssueAction::RemoveJournalKey {
                issue_id: key_owner,
                key,
            }),
            Action::Journal(JournalAction::RemoveUploadingRemote {
                id: journal_id,
                issue_id,
            }),
        ] = actions.as_slice()
        else {
            panic!("reference must be removed before the entry")
        };
        assert_eq!(
            (key_owner, journal_id, issue_id),
            (&42.into(), &1.into(), &42.into())
        );
        assert_eq!(*key, JournalKey::Remote(1.into()));
        assert!(client.put_requests().is_empty());
    }

    #[tokio::test]
    async fn missing_completion_preserves_a_key_added_while_get_was_pending() {
        let dispatcher = edited_dispatcher_with_local();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![journal(2, "other")]));
        let future = upload_remote_journal(dispatcher.clone(), client, 1.into()).unwrap();
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::ReplaceJournalKeys {
                id: 42.into(),
                journal_keys: vec![
                    JournalKey::Remote(1.into()),
                    JournalKey::Remote(2.into()),
                    JournalKey::Remote(3.into()),
                    JournalKey::Local(crate::vos::LocalJournalId::new(1)),
                ],
            });
        dispatcher.borrow_mut().consume_action();

        let actions = future.await;
        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }

        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_issue(42)
                .unwrap()
                .0
                .journal_keys,
            vec![
                JournalKey::Remote(2.into()),
                JournalKey::Remote(3.into()),
                JournalKey::Local(crate::vos::LocalJournalId::new(1)),
            ]
        );
    }

    #[tokio::test]
    async fn completes_from_fetched_journal_when_notes_are_already_applied() {
        let dispatcher = edited_dispatcher();
        let fetched = journal(1, "after");
        let client = Arc::new(StubClient::succeeds(issue(42), vec![fetched.clone()]));

        let actions = upload_remote_journal(dispatcher, client.clone(), 1.into())
            .unwrap()
            .await;

        let [Action::Journal(JournalAction::CompleteRemoteUploadFromFetch { journal, issue_id })] =
            actions.as_slice()
        else {
            panic!("fetched journal must complete the upload")
        };
        assert_eq!(
            (journal.id, journal.notes.as_str(), issue_id),
            (1.into(), "after", &42.into())
        );
        assert_eq!(journal.user, fetched.user);
        assert_eq!(journal.updated_on, fetched.updated_on);
        assert_eq!(journal.details.len(), fetched.details.len());
        assert!(client.put_requests().is_empty());
    }

    #[tokio::test]
    async fn uploads_after_notes_when_server_is_unchanged() {
        let dispatcher = edited_dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![journal(1, "before")]));

        let actions = upload_remote_journal(dispatcher, client.clone(), 1.into())
            .unwrap()
            .await;

        assert_eq!(
            client.put_requests(),
            vec![(JournalId::new(1), "after".to_string())]
        );
        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::CompleteRemoteUpload { id, notes })] if id == &JournalId::new(1) && notes == "after")
        );
    }

    #[tokio::test]
    async fn returns_fail_upload_when_put_fails() {
        let dispatcher = edited_dispatcher();
        let client = Arc::new(StubClient::fails_put(issue(42), vec![journal(1, "before")]));

        let actions = upload_remote_journal(dispatcher, client, 1.into())
            .unwrap()
            .await;

        assert!(matches!(
            actions.as_slice(),
            [Action::Journal(JournalAction::FailUpload { failure, .. })]
                if failure.stage == JournalUploadFailureStage::RemotePut
                    && failure.message == "network error: offline"
        ));
    }

    #[tokio::test]
    async fn reports_conflict_without_putting() {
        let dispatcher = edited_dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![journal(1, "server")]));

        let actions = upload_remote_journal(dispatcher, client.clone(), 1.into())
            .unwrap()
            .await;

        let [Action::Journal(JournalAction::UploadConflictsDetected { conflict })] =
            actions.as_slice()
        else {
            panic!("three changed values must return a conflict action")
        };
        assert_eq!((conflict.id, conflict.issue_id), (1.into(), 42.into()));
        assert_eq!(
            (
                conflict.before.as_str(),
                conflict.after.as_str(),
                conflict.server.notes.as_str()
            ),
            ("before", "after", "server")
        );
        assert_eq!(conflict.server.user, "user1");
        assert!(!conflict.server.details.is_empty());
        assert!(client.put_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_the_issue_is_uploading() {
        let dispatcher = edited_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateDescription {
                id: 42.into(),
                body: "edited".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartUpload { id: 42.into() });
        dispatcher.borrow_mut().consume_action();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![]));

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_remote_journal(dispatcher.clone(), client.clone(), 1.into())
            }))
            .is_err()
        );
        assert!(client.get_requests().is_empty());
        assert!(client.put_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_another_journal_of_the_issue_is_uploading() {
        let dispatcher = edited_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::RegisterRemote {
                journal: journal(2, "other"),
                issue_id: 42.into(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::EditRemoteNotes {
                id: 2.into(),
                notes: "other-edited".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartUpload {
                key: JournalKey::Remote(2.into()),
            });
        dispatcher.borrow_mut().consume_action();
        let client = Arc::new(StubClient::succeeds(issue(42), vec![]));

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_remote_journal(dispatcher.clone(), client.clone(), 1.into())
            }))
            .is_err()
        );
        assert!(client.get_requests().is_empty());
        assert!(client.put_requests().is_empty());
    }

    #[test]
    fn skips_synced_and_uploading_journals_without_http() {
        for uploading in [false, true] {
            let dispatcher = edited_dispatcher();
            if uploading {
                dispatcher
                    .borrow_mut()
                    .dispatch(JournalAction::StartUpload {
                        key: JournalKey::Remote(1.into()),
                    });
                dispatcher.borrow_mut().consume_action();
            } else {
                dispatcher
                    .borrow_mut()
                    .dispatch(JournalAction::EditRemoteNotes {
                        id: 1.into(),
                        notes: "before".to_string(),
                    });
                dispatcher.borrow_mut().consume_action();
            }
            let client = Arc::new(StubClient::succeeds(issue(42), vec![]));
            assert!(upload_remote_journal(dispatcher, client.clone(), 1.into()).is_none());
            assert!(client.get_requests().is_empty());
        }
    }

    fn edited_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut aggregate = issue(42);
        aggregate.journal_keys = vec![JournalKey::Remote(1.into())];
        for action in Vec::<Action>::from([
            IssueAction::Sync { issue: aggregate }.into(),
            JournalAction::RegisterRemote {
                journal: journal(1, "before"),
                issue_id: 42.into(),
            }
            .into(),
            JournalAction::EditRemoteNotes {
                id: 1.into(),
                notes: "after".to_string(),
            }
            .into(),
        ]) {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }
        dispatcher
    }

    fn edited_dispatcher_with_local() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = edited_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::RegisterRemote {
                journal: journal(2, "other"),
                issue_id: 42.into(),
            });
        dispatcher.borrow_mut().consume_action();
        let local_id = dispatcher.borrow_mut().new_local_journal_id();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::CreateLocal {
                id: local_id,
                issue_id: 42.into(),
                notes: "local".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::ReplaceJournalKeys {
                id: 42.into(),
                journal_keys: vec![
                    JournalKey::Remote(1.into()),
                    JournalKey::Remote(2.into()),
                    JournalKey::Local(local_id),
                ],
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
    }

    fn issue(id: u16) -> IssueAggregate {
        sample_issue_aggregate(id, "subject", IssueStatusId::new(1), None, None, None, 0)
    }

    fn journal(id: u16, notes: &str) -> Journal {
        let mut journal = parse_journal_yaml(JournalId::new(id));
        journal.notes = notes.to_string();
        journal
    }

    struct StubClient {
        get_result: Result<(IssueAggregate, Vec<Journal>), RedmineClientError>,
        put_result: Result<(), RedmineClientError>,
        gets: Mutex<Vec<IssueId>>,
        puts: Mutex<Vec<(JournalId, String)>>,
    }

    impl StubClient {
        fn succeeds(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self::new(Ok((issue, journals)), Ok(()))
        }
        fn fails_get() -> Self {
            Self::new(
                Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
                Ok(()),
            )
        }
        fn fails_put(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self::new(
                Ok((issue, journals)),
                Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
            )
        }
        fn new(
            get_result: Result<(IssueAggregate, Vec<Journal>), RedmineClientError>,
            put_result: Result<(), RedmineClientError>,
        ) -> Self {
            Self {
                get_result,
                put_result,
                gets: Mutex::new(vec![]),
                puts: Mutex::new(vec![]),
            }
        }
        fn get_requests(&self) -> Vec<IssueId> {
            self.gets.lock().unwrap().clone()
        }
        fn put_requests(&self) -> Vec<(JournalId, String)> {
            self.puts.lock().unwrap().clone()
        }
    }

    impl RedmineClient for StubClient {
        async fn get_issue(
            &self,
            id: IssueId,
        ) -> Result<(IssueAggregate, Vec<Journal>), RedmineClientError> {
            self.gets.lock().unwrap().push(id);
            self.get_result.clone()
        }
        async fn update_journal_notes(
            &self,
            id: JournalId,
            notes: &str,
        ) -> Result<(), RedmineClientError> {
            self.puts.lock().unwrap().push((id, notes.to_string()));
            self.put_result.clone()
        }
        async fn create_journal(
            &self,
            _: crate::vos::IssueId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            unreachable!()
        }
        async fn update_issue(&self, _: &IssueAggregate) -> Result<(), RedmineClientError> {
            unreachable!()
        }
        async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
            unreachable!()
        }
        async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError> {
            unreachable!()
        }
        async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError> {
            unreachable!()
        }
        async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError> {
            unreachable!()
        }
        async fn get_project_issues(
            &self,
            _: crate::vos::ProjectId,
            _: std::num::NonZeroUsize,
        ) -> Result<crate::entities::ProjectIssuesPage, RedmineClientError> {
            unreachable!()
        }
        async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError> {
            unreachable!()
        }
        async fn get_time_entity_activities(
            &self,
        ) -> Result<Vec<TimeEntityActivity>, RedmineClientError> {
            unreachable!()
        }
        async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError> {
            unreachable!()
        }
        async fn get_users(&self) -> Result<Vec<User>, RedmineClientError> {
            unreachable!()
        }
    }
}
