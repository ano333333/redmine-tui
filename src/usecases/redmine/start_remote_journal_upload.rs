use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, IssueState, JournalAction, RemoteJournalState};
use crate::vos::{IssueId, JournalId, JournalNotesDiff};

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
    unreachable!("remote journal upload HTTP flow is implemented in a later step")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use crate::clients::redmine::base::FetchedIssue;
    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Priority, Project, TargetVersion,
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
        })
    }

    struct StubClient {
        requested: Mutex<bool>,
    }

    impl RedmineClient for StubClient {
        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            *self.requested.lock().unwrap() = true;
            Ok(())
        }

        async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
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
}
