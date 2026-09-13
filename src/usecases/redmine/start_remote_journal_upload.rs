use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, IssueState, JournalAction, RemoteJournalState};
use crate::vos::{EntityIdValue, IssueId, JournalId, JournalNotesDiff};

use super::resolve_remote_journal_upload::{
    RemoteJournalUploadResolution, resolve_remote_journal_upload,
};

/// Remote Journal uploadの完了Actionを生成するFuture。
pub type StartRemoteJournalUploadFuture =
    Pin<Box<dyn Future<Output = JournalAction> + Send + 'static>>;

/// 編集済みのRemote Journalのuploadを開始する。
///
/// `StartRemoteUpload`はFutureをpollする前に同期的にqueueへ追加し、返却したFutureは
/// uploadの完了Actionを返す。
///
/// # Panics
///
/// Issueがupload中、対象JournalがEdited以外、または同じIssueの別Journalがupload中の場合に
/// panicする。
pub fn start_remote_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
    journal_id: JournalId,
) -> StartRemoteJournalUploadFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let diff = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        if matches!(store.get_issue_state(issue_id), Some(IssueState::Uploading)) {
            panic!("cannot start remote journal upload while issue {issue_id} is uploading");
        }
        let entry = store.get_remote_journal(issue_id, journal_id);
        let diff = match &entry.state {
            RemoteJournalState::Edited { diff, .. } => diff.clone(),
            _ => panic!(
                "cannot start remote journal upload unless remote journal {journal_id} is edited"
            ),
        };
        if store
            .get_remote_journals(issue_id)
            .iter()
            .filter(|entry| entry.journal.id != journal_id)
            .any(|entry| matches!(entry.state, RemoteJournalState::Uploading { .. }))
        {
            panic!(
                "cannot start remote journal upload while another journal of issue {issue_id} is uploading"
            );
        }
        diff
    };

    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartRemoteUpload {
            issue_id,
            journal_id,
        });

    Box::pin(
        async move { upload_remote_journal(client.as_ref(), issue_id, journal_id, diff).await },
    )
}

async fn upload_remote_journal<C>(
    client: &C,
    issue_id: IssueId,
    journal_id: JournalId,
    diff: JournalNotesDiff,
) -> JournalAction
where
    C: RedmineClient + Send + Sync + 'static,
{
    let fetched = match client.get_issue(issue_id).await {
        Err(error) => {
            return JournalAction::FailRemoteUpload {
                issue_id,
                journal_id,
                message: error.to_string(),
            };
        }
        Ok(fetched) => fetched,
    };
    if fetched.aggregate.issue.id != issue_id {
        return JournalAction::FailRemoteUpload {
            issue_id,
            journal_id,
            message: format!(
                "requested issue {} but Redmine returned issue {}",
                issue_id.get(),
                fetched.aggregate.issue.id.get()
            ),
        };
    }
    let Some(server_journal) = fetched
        .journals
        .iter()
        .find(|journal| journal.id == journal_id)
    else {
        return JournalAction::RemoveMissingRemoteJournal {
            issue_id,
            journal_id,
        };
    };
    complete_remote_upload(client, issue_id, journal_id, &diff, &server_journal.notes).await
}

async fn complete_remote_upload<C>(
    client: &C,
    issue_id: IssueId,
    journal_id: JournalId,
    diff: &JournalNotesDiff,
    server_notes: &str,
) -> JournalAction
where
    C: RedmineClient + Send + Sync + 'static,
{
    match resolve_remote_journal_upload(&diff.before, &diff.after, server_notes) {
        // 同じ編集内容が既にサーバーへ反映されていれば、重複PUTせず正常完了として収束させる。
        RemoteJournalUploadResolution::AlreadyApplied => JournalAction::CompleteRemoteUpload {
            issue_id,
            journal_id,
            notes: server_notes.to_string(),
        },
        RemoteJournalUploadResolution::Upload => {
            match client.update_journal_notes(journal_id, &diff.after).await {
                Ok(()) => JournalAction::CompleteRemoteUpload {
                    issue_id,
                    journal_id,
                    notes: diff.after.clone(),
                },
                Err(error) => JournalAction::FailRemoteUpload {
                    issue_id,
                    journal_id,
                    message: error.to_string(),
                },
            }
        }
        RemoteJournalUploadResolution::Conflict => JournalAction::FailRemoteUpload {
            issue_id,
            journal_id,
            message: "remote journal conflict resolution is not implemented yet".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use crate::clients::redmine::base::FetchedIssue;
    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Journal, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{
        Action, Dispatcher, IssueAction, IssueState, JournalAction, RemoteJournalState,
    };
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::{IssueId, IssueStatusId, JournalId};

    use super::start_remote_journal_upload;

    const ISSUE_ID: IssueId = IssueId::new(1);
    const JOURNAL_ID: JournalId = JournalId::new(10);

    fn journal() -> crate::entities::Journal {
        crate::entities::Journal {
            id: JOURNAL_ID,
            issue_id: ISSUE_ID,
            user: "alice".to_string(),
            updated_on: local_datetime("2026-09-10T00:00:00+09:00"),
            details: vec![],
            notes: "remote notes".to_string(),
        }
    }

    fn journal_with_notes(notes: &str) -> crate::entities::Journal {
        let mut journal = journal();
        journal.notes = notes.to_string();
        journal
    }

    fn edited_issue_and_journal(dispatcher: &mut Dispatcher) {
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.issue.description = "issue body".to_string();
        dispatcher.dispatch(Action::Issue(IssueAction::Sync { issue }));
        dispatcher.consume_action();
        let journal = journal();
        dispatcher.dispatch(Action::Journal(JournalAction::SyncFetched {
            issue_id: ISSUE_ID,
            journals: vec![journal],
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            notes: "edited notes".to_string(),
        }));
        dispatcher.consume_action();
    }

    fn stub_client() -> Arc<StubClient> {
        Arc::new(StubClient {
            requested: Mutex::new(false),
            requested_notes: Mutex::new(None),
            get_result: Err(RedmineClientError::Network {
                reason: "offline".to_string(),
            }),
            update_result: Ok(()),
            journals: vec![],
        })
    }

    fn stub_client_with_issue(issue_id: u16, journals: Vec<Journal>) -> Arc<StubClient> {
        Arc::new(StubClient {
            requested: Mutex::new(false),
            requested_notes: Mutex::new(None),
            get_result: Ok(sample_issue_aggregate(
                issue_id,
                "subject",
                IssueStatusId::new(1),
                None,
                None,
                None,
                0,
            )),
            update_result: Ok(()),
            journals,
        })
    }

    fn stub_client_with_issue_and_failed_put(
        issue_id: u16,
        journals: Vec<Journal>,
    ) -> Arc<StubClient> {
        Arc::new(StubClient {
            requested: Mutex::new(false),
            requested_notes: Mutex::new(None),
            get_result: Ok(sample_issue_aggregate(
                issue_id,
                "subject",
                IssueStatusId::new(1),
                None,
                None,
                None,
                0,
            )),
            update_result: Err(RedmineClientError::Network {
                reason: "put failed".to_string(),
            }),
            journals,
        })
    }

    struct StubClient {
        requested: Mutex<bool>,
        requested_notes: Mutex<Option<String>>,
        get_result: Result<IssueAggregate, RedmineClientError>,
        update_result: Result<(), RedmineClientError>,
        journals: Vec<Journal>,
    }

    impl RedmineClient for StubClient {
        fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            notes: &str,
        ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
            Box::pin(async move {
                *self.requested.lock().unwrap() = true;
                *self.requested_notes.lock().unwrap() = Some(notes.to_string());
                self.update_result.clone()
            })
        }

        async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
            self.get_result.clone().map(|aggregate| FetchedIssue {
                aggregate,
                journals: self.journals.clone(),
            })
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

    fn assert_edited_journal(dispatcher: &Dispatcher) {
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        match &entry.state {
            RemoteJournalState::Edited { diff, .. } => {
                assert_eq!(diff.before, "remote notes");
                assert_eq!(diff.after, "edited notes");
            }
            other => panic!("expected edited state, got {other:?}"),
        }
    }

    fn assert_panics(dispatcher: &Rc<RefCell<Dispatcher>>) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            start_remote_journal_upload(dispatcher.clone(), stub_client(), ISSUE_ID, JOURNAL_ID);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn start_dispatches_start_remote_upload_and_keeps_the_diff() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        assert_edited_journal(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        let future =
            start_remote_journal_upload(dispatcher.clone(), stub_client(), ISSUE_ID, JOURNAL_ID);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Uploading { diff, conflict } = &entry.state else {
            panic!("expected uploading state");
        };
        assert_eq!(diff.before, "remote notes");
        assert_eq!(diff.after, "edited notes");
        assert!(conflict.is_none());
        drop(future);
    }

    #[test]
    fn start_does_not_call_the_client_until_the_future_is_driven() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client();

        let future =
            start_remote_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID, JOURNAL_ID);

        assert!(!*client.requested.lock().unwrap());
        dispatcher.borrow_mut().consume_action();
        drop(future);
    }

    #[test]
    fn start_panics_when_the_issue_is_uploading() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        dispatcher.dispatch(IssueAction::UpdateDescription {
            id: ISSUE_ID,
            body: "edited issue body".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::StartUpload { id: ISSUE_ID });
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        assert!(matches!(
            dispatcher.borrow().store().get_issue_state(ISSUE_ID),
            Some(IssueState::Uploading)
        ));

        assert_panics(&dispatcher);
    }

    #[test]
    fn start_panics_when_the_journal_is_synced() {
        let mut dispatcher = Dispatcher::new();
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.issue.description = "issue body".to_string();
        dispatcher.dispatch(Action::Issue(IssueAction::Sync { issue }));
        dispatcher.consume_action();
        let journal = journal();
        dispatcher.dispatch(Action::Journal(JournalAction::SyncFetched {
            issue_id: ISSUE_ID,
            journals: vec![journal],
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        assert_panics(&dispatcher);
    }

    #[test]
    fn start_panics_when_another_journal_of_the_issue_is_uploading() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let mut other = journal();
        other.id = JournalId::new(11);
        other.notes = "other notes".to_string();
        dispatcher.dispatch(Action::Journal(JournalAction::SyncFetched {
            issue_id: ISSUE_ID,
            journals: vec![other],
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JournalId::new(11),
            notes: "other edited notes".to_string(),
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID,
            journal_id: JournalId::new(11),
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        assert_panics(&dispatcher);
    }

    #[tokio::test]
    async fn get_failure_returns_fail_remote_upload_and_restores_the_edited_state() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client();

        let action =
            start_remote_journal_upload(dispatcher.clone(), client, ISSUE_ID, JOURNAL_ID).await;
        dispatcher.borrow_mut().consume_action();

        match &action {
            JournalAction::FailRemoteUpload {
                issue_id,
                journal_id,
                message,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(message, "network error: offline");
            }
            _ => panic!("expected fail remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Edited { diff, failure } = &entry.state else {
            panic!("expected edited state");
        };
        assert_eq!(diff.before, "remote notes");
        assert_eq!(diff.after, "edited notes");
        let Some(failure) = failure else {
            panic!("expected failure")
        };
        assert_eq!(failure.message.as_str(), "network error: offline");
    }

    #[tokio::test]
    async fn get_id_mismatch_returns_fail_remote_upload_with_the_requested_and_returned_ids() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(99, vec![journal()]);

        let action =
            start_remote_journal_upload(dispatcher.clone(), client, ISSUE_ID, JOURNAL_ID).await;
        dispatcher.borrow_mut().consume_action();

        match &action {
            JournalAction::FailRemoteUpload {
                issue_id,
                journal_id,
                message,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(message, "requested issue 1 but Redmine returned issue 99");
            }
            _ => panic!("expected fail remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Edited { .. } = &entry.state else {
            panic!("expected edited state");
        };
    }

    #[tokio::test]
    async fn a_journal_missing_from_the_get_result_is_removed_from_the_store() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![]);

        let action =
            start_remote_journal_upload(dispatcher.clone(), client, ISSUE_ID, JOURNAL_ID).await;
        dispatcher.borrow_mut().consume_action();

        match &action {
            JournalAction::RemoveMissingRemoteJournal {
                issue_id,
                journal_id,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
            }
            _ => panic!("expected remove missing remote journal action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        assert!(
            dispatcher
                .borrow()
                .store()
                .get_remote_journals(ISSUE_ID)
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_journal_unchanged_on_the_server_is_put_and_completes_with_the_edited_notes() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![journal()]);

        let action =
            start_remote_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID, JOURNAL_ID)
                .await;
        dispatcher.borrow_mut().consume_action();

        assert_eq!(
            client.requested_notes.lock().unwrap().as_deref(),
            Some("edited notes")
        );
        match &action {
            JournalAction::CompleteRemoteUpload {
                issue_id,
                journal_id,
                notes,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(notes, "edited notes");
            }
            _ => panic!("expected complete remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        assert!(matches!(entry.state, RemoteJournalState::Synced));
        assert_eq!(entry.journal.notes, "edited notes");
        assert_eq!(
            entry.journal.updated_on,
            local_datetime("2026-09-10T00:00:00+09:00")
        );
    }

    #[tokio::test]
    async fn server_equal_to_the_edited_notes_completes_without_a_put() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![journal_with_notes("edited notes")]);

        let action =
            start_remote_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID, JOURNAL_ID)
                .await;
        dispatcher.borrow_mut().consume_action();

        assert!(!*client.requested.lock().unwrap());
        match &action {
            JournalAction::CompleteRemoteUpload { notes, .. } => {
                assert_eq!(notes, "edited notes");
            }
            _ => panic!("expected complete remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        assert!(matches!(entry.state, RemoteJournalState::Synced));
        assert_eq!(entry.journal.notes, "edited notes");
        assert_eq!(
            entry.journal.updated_on,
            local_datetime("2026-09-10T00:00:00+09:00")
        );
    }

    #[tokio::test]
    async fn a_failed_put_returns_fail_remote_upload_and_restores_the_edited_state() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue_and_failed_put(1, vec![journal()]);

        let action =
            start_remote_journal_upload(dispatcher.clone(), client, ISSUE_ID, JOURNAL_ID).await;
        dispatcher.borrow_mut().consume_action();

        match &action {
            JournalAction::FailRemoteUpload {
                issue_id,
                journal_id,
                message,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(message, "network error: put failed");
            }
            _ => panic!("expected fail remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Edited { diff, failure } = &entry.state else {
            panic!("expected edited state");
        };
        assert_eq!(diff.before, "remote notes");
        assert_eq!(diff.after, "edited notes");
        let Some(failure) = failure else {
            panic!("expected failure")
        };
        assert_eq!(failure.message.as_str(), "network error: put failed");
    }

    #[tokio::test]
    async fn a_conflicting_server_notes_returns_the_not_implemented_action() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![journal_with_notes("conflicting notes")]);

        let action =
            start_remote_journal_upload(dispatcher.clone(), client, ISSUE_ID, JOURNAL_ID).await;
        dispatcher.borrow_mut().consume_action();

        match &action {
            JournalAction::FailRemoteUpload { message, .. } => {
                assert_eq!(
                    message,
                    "remote journal conflict resolution is not implemented yet"
                );
            }
            _ => panic!("expected fail remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Edited { .. } = &entry.state else {
            panic!("expected edited state");
        };
    }
}
