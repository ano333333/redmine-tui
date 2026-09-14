use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, Dispatcher, IssueState, JournalAction, LocalJournalState, NoticeAction, NoticeId,
    RemoteJournalState,
};
use crate::vos::IssueId;

/// Local JournalのPUT結果に応じたActionを生成するFuture。
pub type StartLocalJournalUploadFuture =
    Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

/// 未保存のLocal Journalのuploadを開始する。
///
/// `StartLocalUpload`はFutureをpollする前に同期的にqueueへ追加する。返却したFutureは
/// PUT失敗時にnoticeとLocal Journalを再試行可能にするActionを返し、成功時は空のAction列を返す。
///
/// # Panics
///
/// Issueがupload中、対象Local Journalが未登録またはLocalOnly以外、もしくは同じIssueの
/// Remote Journalがupload中の場合にpanicする。
pub fn start_local_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
) -> StartLocalJournalUploadFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let notes = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        if matches!(store.get_issue_state(issue_id), Some(IssueState::Uploading)) {
            panic!("cannot start local journal upload while issue {issue_id} is uploading");
        }
        let entry = store
            .get_local_journal(issue_id)
            .unwrap_or_else(|| panic!("local journal is not registered for issue {issue_id}"));
        if !matches!(entry.state, LocalJournalState::LocalOnly { .. }) {
            panic!("cannot start local journal upload unless it is local only");
        }
        if store
            .get_remote_journals(issue_id)
            .iter()
            .any(|entry| matches!(entry.state, RemoteJournalState::Uploading { .. }))
        {
            panic!(
                "cannot start local journal upload while another journal of issue {issue_id} is uploading"
            );
        }
        entry.journal.notes.clone()
    };

    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartLocalUpload { issue_id });

    Box::pin(async move {
        match client.update_issue_notes(issue_id, &notes).await {
            Ok(()) => vec![],
            Err(error) => {
                let message = error.to_string();
                vec![
                    NoticeAction::Push {
                        id: NoticeId::new(),
                        message: format!("Local Journalの保存に失敗しました: {message}"),
                        created_at: chrono::Local::now(),
                    }
                    .into(),
                    JournalAction::FailLocalUpload { issue_id, message }.into(),
                ]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::clients::redmine::RedmineClientError;
    use crate::clients::redmine::base::FetchedIssue;
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Journal, Priority, Project, ProjectIssuesPage,
        TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{IssueAction, JournalUploadFailure};
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::{IssueStatusId, JournalId, ProjectId};

    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);

    struct StubClient {
        result: Result<(), RedmineClientError>,
        request: Mutex<Option<(IssueId, String)>>,
    }

    impl StubClient {
        fn succeeds() -> Self {
            Self {
                result: Ok(()),
                request: Mutex::new(None),
            }
        }

        fn fails() -> Self {
            Self {
                result: Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
                request: Mutex::new(None),
            }
        }
    }

    impl RedmineClient for StubClient {
        async fn update_issue_notes(
            &self,
            issue_id: IssueId,
            notes: &str,
        ) -> Result<(), RedmineClientError> {
            *self.request.lock().unwrap() = Some((issue_id, notes.to_string()));
            self.result.clone()
        }

        async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
            unreachable!()
        }

        async fn update_issue(&self, _: &IssueAggregate) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_journal_notes(
            &self,
            _: JournalId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
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
            _: ProjectId,
            _: std::num::NonZeroUsize,
        ) -> Result<ProjectIssuesPage, RedmineClientError> {
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

    fn local_only_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(JournalAction::CreateLocal { issue_id: ISSUE_ID });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID,
            notes: "local notes".to_string(),
        });
        dispatcher.consume_action();
        Rc::new(RefCell::new(dispatcher))
    }

    fn assert_start_panics(dispatcher: Rc<RefCell<Dispatcher>>) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            start_local_journal_upload(dispatcher, Arc::new(StubClient::succeeds()), ISSUE_ID)
        }));
        assert!(result.is_err());
    }

    #[test]
    fn start_dispatches_before_the_future_is_polled() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds());

        let future = start_local_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID);

        assert!(client.request.lock().unwrap().is_none());
        dispatcher.borrow_mut().consume_action();
        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_local_journal(ISSUE_ID)
                .unwrap()
                .state,
            LocalJournalState::Uploading
        ));
        drop(future);
    }

    #[test]
    fn start_panics_when_the_local_journal_is_uploading() {
        let dispatcher = local_only_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartLocalUpload { issue_id: ISSUE_ID });
        dispatcher.borrow_mut().consume_action();

        assert_start_panics(dispatcher);
    }

    #[test]
    fn start_panics_when_the_issue_is_uploading() {
        let dispatcher = local_only_dispatcher();
        let issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::Sync { issue });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateDescription {
                id: ISSUE_ID,
                body: "edited".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartUpload { id: ISSUE_ID });
        dispatcher.borrow_mut().consume_action();

        assert_start_panics(dispatcher);
    }

    #[test]
    fn start_panics_when_a_remote_journal_is_uploading() {
        let dispatcher = local_only_dispatcher();
        let journal_id = JournalId::new(10);
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::SyncFetched {
                issue_id: ISSUE_ID,
                journals: vec![Journal {
                    id: journal_id,
                    issue_id: ISSUE_ID,
                    user: "alice".to_string(),
                    updated_on: local_datetime("2026-09-10T00:00:00+09:00"),
                    details: vec![],
                    notes: "remote".to_string(),
                }],
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::EditRemoteNotes {
                issue_id: ISSUE_ID,
                journal_id,
                notes: "edited".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartRemoteUpload {
                issue_id: ISSUE_ID,
                journal_id,
            });
        dispatcher.borrow_mut().consume_action();

        assert_start_panics(dispatcher);
    }

    #[tokio::test]
    async fn put_success_sends_the_local_notes_and_returns_no_completion_action_yet() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds());

        let actions = start_local_journal_upload(dispatcher, client.clone(), ISSUE_ID).await;

        assert!(actions.is_empty());
        assert_eq!(
            *client.request.lock().unwrap(),
            Some((ISSUE_ID, "local notes".to_string()))
        );
    }

    #[tokio::test]
    async fn put_failure_returns_failure_actions_that_restore_notes() {
        let dispatcher = local_only_dispatcher();
        let actions =
            start_local_journal_upload(dispatcher.clone(), Arc::new(StubClient::fails()), ISSUE_ID)
                .await;
        dispatcher.borrow_mut().consume_action();

        assert_eq!(actions.len(), 2);
        let Action::Journal(JournalAction::FailLocalUpload { issue_id, message }) = &actions[1]
        else {
            panic!("expected fail local upload action");
        };
        assert_eq!(*issue_id, ISSUE_ID);
        assert_eq!(message, "network error: offline");
        dispatcher
            .borrow_mut()
            .dispatch(actions.into_iter().nth(1).unwrap());
        dispatcher.borrow_mut().consume_action();
        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_local_journal(ISSUE_ID).unwrap();
        assert_eq!(entry.journal.notes, "local notes");
        assert_eq!(
            entry.state,
            LocalJournalState::LocalOnly {
                failure: Some(JournalUploadFailure {
                    message: "network error: offline".to_string()
                })
            }
        );
    }
}
