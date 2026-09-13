use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, JournalAction, RemoteJournalState};
use crate::vos::{EntityIdValue, IssueId, JournalId};

use super::resolve_remote_journal_upload::{
    RemoteJournalUploadResolution, resolve_remote_journal_upload,
};

/// 競合解決後のRemote Journal upload継続の完了Actionを生成するFuture。
pub type ContinueRemoteJournalUploadFuture =
    Pin<Box<dyn Future<Output = JournalAction> + Send + 'static>>;

/// 競合解決後のRemote Journal uploadを継続する。
///
/// # Panics
///
/// 対象Remote Journalが `Uploading { conflict: Some(_) }` でない場合にpanicする。
pub fn continue_remote_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
    journal_id: JournalId,
    resolved_notes: String,
) -> ContinueRemoteJournalUploadFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let conflict_server_notes = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        let entry = store.get_remote_journal(issue_id, journal_id);
        match &entry.state {
            RemoteJournalState::Uploading {
                conflict: Some(conflict),
                ..
            } => conflict.server_notes.clone(),
            _ => panic!(
                "cannot continue remote journal upload unless remote journal {journal_id} is in conflict"
            ),
        }
    };

    Box::pin(async move {
        continue_remote_journal_upload_inner(
            client.as_ref(),
            issue_id,
            journal_id,
            &conflict_server_notes,
            &resolved_notes,
        )
        .await
    })
}

async fn continue_remote_journal_upload_inner<C>(
    client: &C,
    issue_id: IssueId,
    journal_id: JournalId,
    conflict_server_notes: &str,
    resolved_notes: &str,
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

    // 前回の競合検出時点からのサーバー更新も検出するため、その時点の値を比較の基準にする。
    match resolve_remote_journal_upload(
        conflict_server_notes,
        resolved_notes,
        &server_journal.notes,
    ) {
        // 解決値がすでにサーバーへ反映済みなら、重複PUTせず正常完了として収束させる。
        RemoteJournalUploadResolution::AlreadyApplied => JournalAction::CompleteRemoteUpload {
            issue_id,
            journal_id,
            notes: resolved_notes.to_string(),
        },
        RemoteJournalUploadResolution::Upload => {
            match client
                .update_journal_notes(journal_id, resolved_notes)
                .await
            {
                Ok(()) => JournalAction::CompleteRemoteUpload {
                    issue_id,
                    journal_id,
                    notes: resolved_notes.to_string(),
                },
                Err(error) => JournalAction::FailRemoteUpload {
                    issue_id,
                    journal_id,
                    message: error.to_string(),
                },
            }
        }
        RemoteJournalUploadResolution::Conflict => JournalAction::DetectRemoteUploadConflict {
            issue_id,
            journal_id,
            server_notes: server_journal.notes.clone(),
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
    use crate::stores::{Action, Dispatcher, IssueAction, JournalAction, RemoteJournalState};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId};

    use super::continue_remote_journal_upload;

    const ISSUE_ID: IssueId = IssueId::new(1);
    const JOURNAL_ID: JournalId = JournalId::new(10);

    const ORIGINAL_SERVER: &str = "remote notes";
    const CONFLICT_SERVER: &str = "conflicting notes";

    fn journal() -> crate::entities::Journal {
        crate::entities::Journal {
            id: JOURNAL_ID,
            issue_id: ISSUE_ID,
            user: "alice".to_string(),
            updated_on: crate::test_support::local_datetime("2026-09-10T00:00:00+09:00"),
            details: vec![],
            notes: ORIGINAL_SERVER.to_string(),
        }
    }

    fn journal_with_notes(notes: &str) -> crate::entities::Journal {
        let mut journal = journal();
        journal.notes = notes.to_string();
        journal
    }

    fn conflict_dispatcher(dispatcher: &mut Dispatcher) {
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
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::DetectRemoteUploadConflict {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            server_notes: CONFLICT_SERVER.to_string(),
        }));
        dispatcher.consume_action();
    }

    fn assert_conflict_state(dispatcher: &Dispatcher) {
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        match &entry.state {
            RemoteJournalState::Uploading { conflict, .. } => {
                let Some(conflict) = conflict else {
                    panic!("expected conflict");
                };
                assert_eq!(conflict.server_notes, CONFLICT_SERVER);
            }
            other => panic!("expected uploading with conflict, got {other:?}"),
        }
    }

    struct StubClient {
        requested: Mutex<bool>,
        requested_notes: Mutex<Option<String>>,
        get_result: Result<IssueAggregate, RedmineClientError>,
        update_result: Result<(), RedmineClientError>,
        journals: Vec<Journal>,
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

    fn stub_client_with_failed_put(issue_id: u16, journals: Vec<Journal>) -> Arc<StubClient> {
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

    fn assert_panics(dispatcher: &Rc<RefCell<Dispatcher>>) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            continue_remote_journal_upload(
                dispatcher.clone(),
                stub_client(),
                ISSUE_ID,
                JOURNAL_ID,
                "resolved notes".to_string(),
            );
        }));
        assert!(result.is_err());
    }

    #[test]
    fn panics_when_the_journal_is_synced() {
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
    fn panics_when_the_journal_is_edited() {
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
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            notes: "edited notes".to_string(),
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        assert_panics(&dispatcher);
    }

    #[test]
    fn panics_when_uploading_without_conflict() {
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
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            notes: "edited notes".to_string(),
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        assert_panics(&dispatcher);
    }

    #[tokio::test]
    async fn get_failure_returns_fail_remote_upload() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client();

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client,
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
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
        match &entry.state {
            RemoteJournalState::Edited { diff, failure } => {
                assert_eq!(diff.before, ORIGINAL_SERVER);
                assert_eq!(diff.after, "edited notes");
                let Some(failure) = failure else {
                    panic!("expected failure")
                };
                assert_eq!(failure.message.as_str(), "network error: offline");
            }
            other => panic!("expected edited state, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_id_mismatch_returns_fail_remote_upload() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(99, vec![journal()]);

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client,
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
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
        assert!(matches!(entry.state, RemoteJournalState::Edited { .. }));
    }

    #[tokio::test]
    async fn a_journal_missing_from_the_get_result_is_removed_from_the_store() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![]);

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client,
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
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
    async fn server_equal_to_resolved_notes_completes_without_a_put() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![journal_with_notes("resolved notes")]);

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client.clone(),
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
        dispatcher.borrow_mut().consume_action();

        assert!(!*client.requested.lock().unwrap());
        match &action {
            JournalAction::CompleteRemoteUpload {
                issue_id,
                journal_id,
                notes,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(notes, "resolved notes");
            }
            _ => panic!("expected complete remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        assert!(matches!(entry.state, RemoteJournalState::Synced));
        assert_eq!(entry.journal.notes, "resolved notes");
    }

    #[tokio::test]
    async fn server_equal_to_conflict_server_notes_puts_and_completes() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![journal_with_notes(CONFLICT_SERVER)]);

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client.clone(),
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
        dispatcher.borrow_mut().consume_action();

        assert_eq!(
            client.requested_notes.lock().unwrap().as_deref(),
            Some("resolved notes")
        );
        match &action {
            JournalAction::CompleteRemoteUpload {
                issue_id,
                journal_id,
                notes,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(notes, "resolved notes");
            }
            _ => panic!("expected complete remote upload action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        assert!(matches!(entry.state, RemoteJournalState::Synced));
        assert_eq!(entry.journal.notes, "resolved notes");
    }

    #[tokio::test]
    async fn a_failed_put_returns_fail_remote_upload() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_failed_put(1, vec![journal_with_notes(CONFLICT_SERVER)]);

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client,
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
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
        match &entry.state {
            RemoteJournalState::Edited { diff, failure } => {
                assert_eq!(diff.before, ORIGINAL_SERVER);
                assert_eq!(diff.after, "edited notes");
                let Some(failure) = failure else {
                    panic!("expected failure")
                };
                assert_eq!(failure.message.as_str(), "network error: put failed");
            }
            other => panic!("expected edited state, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_changed_server_returns_detect_remote_upload_conflict() {
        let mut dispatcher = Dispatcher::new();
        conflict_dispatcher(&mut dispatcher);
        assert_conflict_state(&dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = stub_client_with_issue(1, vec![journal_with_notes("newly changed notes")]);

        let action = continue_remote_journal_upload(
            dispatcher.clone(),
            client,
            ISSUE_ID,
            JOURNAL_ID,
            "resolved notes".to_string(),
        )
        .await;
        dispatcher.borrow_mut().consume_action();

        match &action {
            JournalAction::DetectRemoteUploadConflict {
                issue_id,
                journal_id,
                server_notes,
            } => {
                assert_eq!(*issue_id, ISSUE_ID);
                assert_eq!(*journal_id, JOURNAL_ID);
                assert_eq!(server_notes, "newly changed notes");
            }
            _ => panic!("expected detect remote upload conflict action"),
        }

        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        match &entry.state {
            RemoteJournalState::Uploading { conflict, .. } => {
                let Some(conflict) = conflict else {
                    panic!("expected conflict");
                };
                assert_eq!(conflict.server_notes, "newly changed notes");
            }
            other => panic!("expected uploading state, got {other:?}"),
        }
    }
}
