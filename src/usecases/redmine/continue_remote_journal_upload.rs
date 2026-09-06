//! Continues a Remote Journal upload after a conflict: prepares the Store
//! synchronously, then re-fetches and re-uploads the final notes.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, Dispatcher, IssueAction, JournalAction, JournalUploadFailure,
    JournalUploadFailureStage, RemoteJournalUploadConflict,
};
use crate::vos::{IssueId, JournalId, JournalKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteJournalUploadRetry {
    pub id: JournalId,
    pub issue_id: IssueId,
    pub final_notes: String,
    pub displayed_server_notes: String,
}

/// 競合解決後のnotesをStoreへ反映し、再試行に必要なsnapshotを返す。
pub fn continue_remote_journal_upload(
    dispatcher: &mut Dispatcher,
    id: JournalId,
    final_notes: String,
) -> RemoteJournalUploadRetry {
    let conflict = dispatcher
        .store()
        .get_remote_journal_upload_conflict(id)
        .expect("Remote Journalのアップロード続行には競合情報が必要です");
    let (retry, actions) = continue_remote_journal_upload_actions(conflict, final_notes);
    for action in actions {
        dispatcher.dispatch(action);
    }
    retry
}

fn continue_remote_journal_upload_actions(
    conflict: &RemoteJournalUploadConflict,
    final_notes: String,
) -> (RemoteJournalUploadRetry, [JournalAction; 2]) {
    let retry = RemoteJournalUploadRetry {
        id: conflict.id,
        issue_id: conflict.issue_id,
        final_notes: final_notes.clone(),
        displayed_server_notes: conflict.server.notes.clone(),
    };
    let actions = [
        JournalAction::UpdateUploadingRemoteNotes {
            id: conflict.id,
            notes: final_notes,
        },
        JournalAction::ClearRemoteUploadConflict {
            id: conflict.id,
            issue_id: conflict.issue_id,
        },
    ];
    (retry, actions)
}

pub type RetryRemoteJournalUploadFuture =
    Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

/// 競合続行後の再確認GET/PUTを実行し、完了Action列だけを返す。
pub fn retry_remote_journal_upload<C>(
    client: Arc<C>,
    retry: RemoteJournalUploadRetry,
) -> RetryRemoteJournalUploadFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let RemoteJournalUploadRetry {
        id,
        issue_id,
        final_notes,
        displayed_server_notes,
    } = retry;
    Box::pin(async move {
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
                IssueAction::RemoveJournalKey {
                    issue_id,
                    key: JournalKey::Remote(id),
                }
                .into(),
                JournalAction::RemoveUploadingRemote { id, issue_id }.into(),
            ];
        };
        if final_notes == server.notes {
            return vec![
                JournalAction::CompleteRemoteUploadFromFetch {
                    journal: server,
                    issue_id,
                }
                .into(),
            ];
        }
        if server.notes == displayed_server_notes {
            if let Err(error) = client.update_journal_notes(id, &final_notes).await {
                return failed(id, JournalUploadFailureStage::RemotePut, error.to_string());
            }
            return vec![
                JournalAction::CompleteRemoteUpload {
                    id,
                    notes: final_notes,
                }
                .into(),
            ];
        }
        vec![
            JournalAction::UploadConflictsDetected {
                conflict: RemoteJournalUploadConflict {
                    id,
                    issue_id,
                    before: displayed_server_notes,
                    after: final_notes,
                    server,
                },
            }
            .into(),
        ]
    })
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
    use std::sync::{Arc, Mutex};

    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Journal, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{
        Action, Dispatcher, IssueAction, JournalAction, JournalUploadFailureStage,
        RemoteJournalUploadConflict,
    };
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{
        IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId, JournalKey,
    };

    use super::{
        RemoteJournalUploadRetry, continue_remote_journal_upload,
        continue_remote_journal_upload_actions, retry_remote_journal_upload,
    };

    #[test]
    fn generates_update_then_clear_actions_and_exact_retry() {
        let dispatcher = conflicted_dispatcher();
        let conflict = dispatcher
            .store()
            .get_remote_journal_upload_conflict(1.into())
            .unwrap();

        let (retry, actions) =
            continue_remote_journal_upload_actions(conflict, "final notes".to_string());

        assert_eq!(retry.id, JournalId::new(1));
        assert_eq!(retry.issue_id, IssueId::new(3));
        assert_eq!(retry.final_notes, "final notes");
        assert_eq!(retry.displayed_server_notes, "server notes");
        let [
            JournalAction::UpdateUploadingRemoteNotes { id, notes },
            JournalAction::ClearRemoteUploadConflict {
                id: clear_id,
                issue_id,
            },
        ] = actions
        else {
            panic!("continue actions must update notes then clear the conflict")
        };
        assert_eq!((id, notes), (1.into(), "final notes".to_string()));
        assert_eq!((clear_id, issue_id), (1.into(), 3.into()));
    }

    #[test]
    #[should_panic(expected = "Remote Journalのアップロード続行には競合情報が必要です")]
    fn missing_conflict_is_an_internal_error() {
        continue_remote_journal_upload(
            &mut Dispatcher::new(),
            JournalId::new(1),
            "notes".to_string(),
        );
    }

    #[tokio::test]
    async fn returns_fail_upload_when_get_fails() {
        let actions = retry_remote_journal_upload(Arc::new(StubClient::fails_get()), retry()).await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { key, failure })]
                if key == &JournalKey::Remote(JournalId::new(1))
                    && failure.stage == JournalUploadFailureStage::RemoteFetch
                    && failure.message == "network error: offline")
        );
    }

    #[tokio::test]
    async fn returns_fail_upload_when_get_returns_another_issue() {
        let actions = retry_remote_journal_upload(
            Arc::new(StubClient::succeeds(
                issue(99),
                vec![journal(1, "server notes")],
            )),
            retry(),
        )
        .await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { key, .. })]
                if key == &JournalKey::Remote(JournalId::new(1))
                    && matches!(&actions[0], Action::Journal(JournalAction::FailUpload { failure, .. })
                        if failure.stage == JournalUploadFailureStage::RemoteFetch
                            && failure.message.contains("does not match expected")))
        );
    }

    #[tokio::test]
    async fn removes_a_missing_journal_reference_before_the_entry() {
        let client = Arc::new(StubClient::succeeds(issue(42), vec![journal(2, "other")]));

        let actions = retry_remote_journal_upload(client.clone(), retry()).await;

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
    async fn completes_from_fetch_when_final_notes_match_the_latest_server() {
        let fetched = journal(1, "final notes");
        let client = Arc::new(StubClient::succeeds(issue(42), vec![fetched.clone()]));

        let actions = retry_remote_journal_upload(client.clone(), retry()).await;

        let [Action::Journal(JournalAction::CompleteRemoteUploadFromFetch { journal, issue_id })] =
            actions.as_slice()
        else {
            panic!("matching final notes must complete from the fetched journal")
        };
        assert_eq!(issue_id, &IssueId::new(42));
        assert_eq!(journal.id, JournalId::new(1));
        assert_eq!(journal.user, fetched.user);
        assert_eq!(journal.updated_on, fetched.updated_on);
        assert_eq!(journal.notes, "final notes");
        assert_fixture_details(&journal.details);
        assert!(client.put_requests().is_empty());
    }

    #[tokio::test]
    async fn puts_final_notes_when_the_server_is_unchanged_since_the_conflict() {
        let client = Arc::new(StubClient::succeeds(
            issue(42),
            vec![journal(1, "server notes")],
        ));

        let actions = retry_remote_journal_upload(client.clone(), retry()).await;

        assert_eq!(client.get_requests(), vec![IssueId::new(42)]);
        assert_eq!(
            client.put_requests(),
            vec![(JournalId::new(1), "final notes".to_string())]
        );
        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::CompleteRemoteUpload { id, notes })]
                if id == &JournalId::new(1) && notes == "final notes")
        );
    }

    #[tokio::test]
    async fn returns_fail_upload_when_put_fails() {
        let client = Arc::new(StubClient::fails_put(
            issue(42),
            vec![journal(1, "server notes")],
        ));

        let actions = retry_remote_journal_upload(client, retry()).await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { key, failure })]
                if key == &JournalKey::Remote(JournalId::new(1))
                    && failure.stage == JournalUploadFailureStage::RemotePut
                    && failure.message == "network error: offline")
        );
    }

    #[tokio::test]
    async fn reports_a_new_conflict_when_the_server_changed_again() {
        let latest = journal(1, "new server");
        let client = Arc::new(StubClient::succeeds(issue(42), vec![latest.clone()]));

        let actions = retry_remote_journal_upload(client.clone(), retry()).await;

        let [Action::Journal(JournalAction::UploadConflictsDetected { conflict })] =
            actions.as_slice()
        else {
            panic!("a changed server must return a conflict action")
        };
        assert_eq!((conflict.id, conflict.issue_id), (1.into(), 42.into()));
        assert_eq!(
            (conflict.before.as_str(), conflict.after.as_str()),
            ("server notes", "final notes")
        );
        assert_eq!(conflict.server.id, JournalId::new(1));
        assert_eq!(conflict.server.user, latest.user);
        assert_eq!(conflict.server.updated_on, latest.updated_on);
        assert_eq!(conflict.server.notes, "new server");
        assert_fixture_details(&conflict.server.details);
        assert!(client.put_requests().is_empty());
    }

    fn conflicted_dispatcher() -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(crate::stores::IssueAction::Load { id: 3.into() });
        dispatcher.consume_action();
        dispatcher.dispatch(crate::stores::Action::LoadJournal { id: 1.into() });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditRemoteNotes {
            id: 1.into(),
            notes: "local notes".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::StartUpload {
            key: JournalKey::Remote(1.into()),
        });
        dispatcher.consume_action();
        let mut server = parse_journal_yaml(1.into());
        server.notes = "server notes".to_string();
        dispatcher.dispatch(JournalAction::UploadConflictsDetected {
            conflict: RemoteJournalUploadConflict {
                id: 1.into(),
                issue_id: 3.into(),
                before: "before".to_string(),
                after: "local notes".to_string(),
                server,
            },
        });
        dispatcher.consume_action();
        dispatcher
    }

    fn retry() -> RemoteJournalUploadRetry {
        RemoteJournalUploadRetry {
            id: JournalId::new(1),
            issue_id: IssueId::new(42),
            final_notes: "final notes".to_string(),
            displayed_server_notes: "server notes".to_string(),
        }
    }

    fn issue(id: u16) -> IssueAggregate {
        sample_issue_aggregate(id, "subject", IssueStatusId::new(1), None, None, None, 0)
    }

    fn journal(id: u16, notes: &str) -> Journal {
        let mut journal = parse_journal_yaml(JournalId::new(id));
        journal.notes = notes.to_string();
        journal
    }

    /// datas/journals/1.ymlのdetailsはStatusId 1→2のattr単一件なので、
    /// variantと値までpattern matchして誤ったdetail payloadでも失敗させる。
    fn assert_fixture_details(details: &[JournalDetail]) {
        let [JournalDetail::Attr(JournalDetailAttr::StatusId { old, new })] = details else {
            panic!("fixture journal details must be the single status change")
        };
        assert_eq!((*old, *new), (IssueStatusId::new(1), IssueStatusId::new(2)));
    }

    struct StubClient {
        get_result: Result<(IssueAggregate, Vec<Journal>), RedmineClientError>,
        put_result: Result<(), RedmineClientError>,
        gets: Mutex<Vec<IssueId>>,
        puts: Mutex<Vec<(JournalId, String)>>,
    }

    impl StubClient {
        fn succeeds(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self {
                get_result: Ok((issue, journals)),
                put_result: Ok(()),
                gets: Mutex::new(vec![]),
                puts: Mutex::new(vec![]),
            }
        }
        fn fails_get() -> Self {
            Self {
                get_result: Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
                put_result: Ok(()),
                gets: Mutex::new(vec![]),
                puts: Mutex::new(vec![]),
            }
        }
        fn fails_put(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self {
                get_result: Ok((issue, journals)),
                put_result: Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
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
