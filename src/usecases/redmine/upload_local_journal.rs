//! Uploads a Local Journal to Redmine as a new journal and refreshes the Issue.

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, Dispatcher, IssueAction, IssueState, JournalAction, JournalEntry, JournalUploadFailure,
    JournalUploadFailureStage, LocalJournalState, merge_fetched_journals,
};
use crate::vos::{JournalKey, LocalJournalId};

pub type UploadLocalJournalFuture = Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

/// Starts uploading the Local Journal identified by `id` and returns a future.
///
/// The target must exist, be owned by an Issue that references it, and be
/// `LocalOnly`; the Issue must exist and not be `Uploading`, and no other
/// Journal of the Issue may be `Uploading`.
///
/// `StartUpload` is dispatched synchronously before the future is
/// returned. The future sends the notes-only PUT first, then fetches the Issue.
///
/// - A PUT failure returns one `FailUpload { failure: LocalPut }`.
/// - A GET failure or Issue ID mismatch returns one `FailUpload { failure: LocalRefresh }` whose
/// message notes that retrying may create a duplicate journal.
pub fn upload_local_journal<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: LocalJournalId,
) -> UploadLocalJournalFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let key = JournalKey::Local(id);
    let (issue_id, notes, journal_keys, snapshot_entries) = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        let entry = store
            .get_journal_entry(key)
            .expect("local journal does not exist");
        let (journal, state) = match &entry {
            JournalEntry::Local { journal, state } => (journal, state),
            _ => panic!("local journal key does not resolve to a local entry"),
        };
        let issue_id = journal.issue_id;
        if *state != LocalJournalState::LocalOnly {
            panic!("local journal is not LocalOnly");
        }
        let (issue, issue_state) = store
            .get_issue(issue_id)
            .expect("issue of the local journal does not exist");
        if !issue.journal_keys.contains(&key) {
            panic!("issue does not reference the local journal");
        }
        if *issue_state == IssueState::Uploading {
            panic!("issue is already uploading");
        }
        if store.has_uploading_journal(issue_id) {
            panic!("another journal of the issue is already uploading");
        }
        let mut snapshot_entries = HashMap::with_capacity(issue.journal_keys.len());
        for key in &issue.journal_keys {
            snapshot_entries.insert(
                *key,
                store
                    .get_journal_entry(*key)
                    .expect("issue journal key must resolve before local journal upload")
                    .clone(),
            );
        }
        (
            issue_id,
            journal.notes.clone(),
            issue.journal_keys.clone(),
            snapshot_entries,
        )
    };
    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartUpload { key });

    Box::pin(async move {
        if let Err(error) = client.create_journal(issue_id, &notes).await {
            return vec![to_local_upload_failed_action(
                key,
                JournalUploadFailureStage::LocalPut,
                error.to_string(),
            )];
        }
        let (issue, journals) = match client.get_issue(issue_id).await {
            Ok(result) => result,
            Err(error) => {
                return vec![to_local_upload_failed_action(
                    key,
                    JournalUploadFailureStage::LocalRefresh,
                    format!(
                        "issue fetch failed after journal PUT; retrying may create a duplicate journal: {}",
                        error
                    ),
                )];
            }
        };
        if issue.issue.id != issue_id {
            return vec![to_local_upload_failed_action(
                key,
                JournalUploadFailureStage::LocalRefresh,
                format!(
                    "fetched issue ID {} does not match expected {} after journal PUT; retrying may create a duplicate journal",
                    issue.issue.id, issue_id
                ),
            )];
        }
        let merged =
            merge_fetched_journals(issue_id, journals.clone(), &journal_keys, &snapshot_entries);
        let mut actions: Vec<Action> = Vec::new();
        for journal in journals {
            actions.push(JournalAction::SyncFetchedRemote { journal, issue_id }.into());
        }
        actions.push(
            IssueAction::ReplaceJournalKeys {
                id: issue_id,
                journal_keys: merged
                    .journal_keys
                    .iter()
                    .filter(|key| !matches!(key, JournalKey::Local(_)))
                    .cloned()
                    .collect(),
            }
            .into(),
        );
        let obsolete = journal_keys.iter().filter_map(|key| match key {
            JournalKey::Remote(id) if !merged.entries.contains_key(key) => Some(*id),
            _ => None,
        });
        for id in obsolete {
            actions.push(JournalAction::RemoveSyncedRemote { id, issue_id }.into());
        }
        actions.push(JournalAction::RemoveUploadingLocal { id, issue_id }.into());
        actions
    })
}

fn to_local_upload_failed_action(
    key: JournalKey,
    stage: JournalUploadFailureStage,
    message: String,
) -> Action {
    JournalAction::FailUpload {
        key,
        failure: JournalUploadFailure { stage, message },
    }
    .into()
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
    use crate::vos::{IssueId, IssueStatusId, JournalId, JournalKey, LocalJournalId};

    use super::upload_local_journal;

    fn fixture() -> (Rc<RefCell<Dispatcher>>, LocalJournalId) {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut aggregate = issue(42);
        aggregate.journal_keys = vec![];
        let local_id = dispatcher.borrow_mut().new_local_journal_id();
        for action in Vec::<Action>::from([
            IssueAction::Sync { issue: aggregate }.into(),
            JournalAction::RegisterRemote {
                journal: journal(1, "before"),
                issue_id: 42.into(),
            }
            .into(),
            JournalAction::EditRemoteNotes {
                id: 1.into(),
                notes: "edited".to_string(),
            }
            .into(),
            JournalAction::RegisterRemote {
                journal: journal(2, "other"),
                issue_id: 42.into(),
            }
            .into(),
            JournalAction::EditRemoteNotes {
                id: 2.into(),
                notes: "other-edited".to_string(),
            }
            .into(),
            JournalAction::RegisterRemote {
                journal: journal(3, "synced"),
                issue_id: 42.into(),
            }
            .into(),
            JournalAction::RegisterRemote {
                journal: journal(4, "synced"),
                issue_id: 42.into(),
            }
            .into(),
            JournalAction::CreateLocal {
                id: local_id,
                issue_id: 42.into(),
                notes: "local notes".to_string(),
            }
            .into(),
            IssueAction::ReplaceJournalKeys {
                id: 42.into(),
                journal_keys: vec![
                    JournalKey::Remote(1.into()),
                    JournalKey::Remote(2.into()),
                    JournalKey::Remote(3.into()),
                    JournalKey::Remote(4.into()),
                    JournalKey::Local(local_id),
                ],
            }
            .into(),
        ]) {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }
        (dispatcher, local_id)
    }

    fn fetch() -> Vec<Journal> {
        vec![journal(1, "server"), journal(4, "fresh")]
    }

    #[tokio::test]
    async fn starts_upload_synchronously_before_the_put_request() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::succeeds());

        let _future = upload_local_journal(dispatcher.clone(), client.clone(), local_id);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    #[tokio::test]
    async fn puts_the_local_notes_before_fetching_the_issue() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::succeeds());

        let actions = upload_local_journal(dispatcher, client.clone(), local_id).await;

        assert_eq!(
            client.put_requests(),
            vec![(IssueId::new(42), "local notes".to_string())]
        );
        assert_eq!(client.get_requests(), vec![IssueId::new(42)]);
        assert_eq!(client.call_order(), vec!["put", "get"]);
        assert!(!actions.is_empty());
    }

    #[tokio::test]
    async fn returns_fetched_keys_removal_and_local_removal_actions_in_order() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::succeeds());

        let actions = upload_local_journal(dispatcher, client.clone(), local_id).await;

        let [
            Action::Journal(JournalAction::SyncFetchedRemote {
                journal: first,
                issue_id: first_issue_id,
            }),
            Action::Journal(JournalAction::SyncFetchedRemote {
                journal: second,
                issue_id: second_issue_id,
            }),
            Action::Issue(IssueAction::ReplaceJournalKeys {
                id: keys_issue_id,
                journal_keys,
            }),
            Action::Journal(JournalAction::RemoveSyncedRemote {
                id: obsolete,
                issue_id: obsolete_issue_id,
            }),
            Action::Journal(JournalAction::RemoveUploadingLocal {
                id: removed,
                issue_id: removed_issue_id,
            }),
        ] = actions.as_slice()
        else {
            panic!("unexpected final actions (len {})", actions.len())
        };
        assert_eq!(
            (first.id, first.notes.as_str()),
            (JournalId::new(1), "server")
        );
        assert_eq!(
            (second.id, second.notes.as_str()),
            (JournalId::new(4), "fresh")
        );
        assert_eq!(
            (*first_issue_id, *second_issue_id),
            (IssueId::new(42), IssueId::new(42))
        );
        // Fetched order first, then the Edited remote missing from the fetch
        // in its previous relative position; the Local key is excluded.
        assert_eq!(*keys_issue_id, IssueId::new(42));
        assert_eq!(
            journal_keys,
            &vec![
                JournalKey::Remote(JournalId::new(1)),
                JournalKey::Remote(JournalId::new(4)),
                JournalKey::Remote(JournalId::new(2)),
            ]
        );
        assert_eq!(
            (*obsolete, *obsolete_issue_id),
            (JournalId::new(3), IssueId::new(42))
        );
        assert_eq!((*removed, *removed_issue_id), (local_id, 42.into()));
    }

    #[tokio::test]
    async fn returns_fail_upload_local_put_when_the_put_fails() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::fails_put());

        let actions = upload_local_journal(dispatcher, client.clone(), local_id).await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { key: JournalKey::Local(id), failure })]
                if id == &local_id
                    && failure.stage == JournalUploadFailureStage::LocalPut
                    && failure.message == "network error: offline")
        );
        assert!(client.get_requests().is_empty());
    }

    #[tokio::test]
    async fn returns_fail_upload_local_refresh_when_the_get_fails() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::fails_get());

        let actions = upload_local_journal(dispatcher, client.clone(), local_id).await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { failure, .. })]
                if failure.stage == JournalUploadFailureStage::LocalRefresh
                    && failure.message.contains("retrying may create a duplicate journal")
                    && failure.message.contains("network error: offline"))
        );
    }

    #[tokio::test]
    async fn returns_fail_upload_local_refresh_when_the_issue_id_mismatches() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::succeeds_with(issue(99), fetch()));

        let actions = upload_local_journal(dispatcher, client.clone(), local_id).await;

        assert!(
            matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { failure, .. })]
                if failure.stage == JournalUploadFailureStage::LocalRefresh
                    && failure.message.contains("does not match expected")
                    && failure.message.contains("retrying may create a duplicate journal"))
        );
    }

    #[tokio::test]
    async fn retry_resends_the_same_put_and_may_duplicate_the_journal() {
        let (dispatcher, local_id) = fixture();
        let client = Arc::new(StubClient::fails_put());

        let first = upload_local_journal(dispatcher.clone(), client.clone(), local_id).await;
        let second = upload_local_journal(dispatcher.clone(), client.clone(), local_id).await;

        for actions in [&first, &second] {
            assert!(
                matches!(actions.as_slice(), [Action::Journal(JournalAction::FailUpload { failure, .. })]
                    if failure.stage == JournalUploadFailureStage::LocalPut)
            );
        }
        assert_eq!(
            client.put_requests(),
            vec![
                (IssueId::new(42), "local notes".to_string()),
                (IssueId::new(42), "local notes".to_string())
            ]
        );
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_the_issue_is_uploading() {
        let (dispatcher, local_id) = fixture();
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
        let client = Arc::new(StubClient::succeeds());

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_local_journal(dispatcher.clone(), client.clone(), local_id)
            }))
            .is_err()
        );
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_another_journal_of_the_issue_is_uploading() {
        let (dispatcher, local_id) = fixture();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartUpload {
                key: JournalKey::Remote(1.into()),
            });
        dispatcher.borrow_mut().consume_action();
        let client = Arc::new(StubClient::succeeds());

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_local_journal(dispatcher.clone(), client.clone(), local_id)
            }))
            .is_err()
        );
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_the_local_entry_is_not_referenced_by_the_issue() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut aggregate = issue(42);
        aggregate.journal_keys = vec![JournalKey::Remote(1.into())];
        let local_id = dispatcher.borrow_mut().new_local_journal_id();
        for action in Vec::<Action>::from([
            IssueAction::Sync { issue: aggregate }.into(),
            JournalAction::RegisterRemote {
                journal: journal(1, "before"),
                issue_id: 42.into(),
            }
            .into(),
            JournalAction::CreateLocal {
                id: local_id,
                issue_id: 42.into(),
                notes: "local notes".to_string(),
            }
            .into(),
        ]) {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }
        let client = Arc::new(StubClient::succeeds());

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_local_journal(dispatcher.clone(), client.clone(), local_id)
            }))
            .is_err()
        );
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_the_owner_issue_is_missing() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let local_id = dispatcher.borrow_mut().new_local_journal_id();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::CreateLocal {
                id: local_id,
                issue_id: 42.into(),
                notes: "local notes".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        let client = Arc::new(StubClient::succeeds());

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_local_journal(dispatcher.clone(), client.clone(), local_id)
            }))
            .is_err()
        );
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_the_local_entry_is_already_uploading() {
        let (dispatcher, local_id) = fixture();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartUpload {
                key: JournalKey::Local(local_id),
            });
        dispatcher.borrow_mut().consume_action();
        let client = Arc::new(StubClient::succeeds());

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_local_journal(dispatcher.clone(), client.clone(), local_id)
            }))
            .is_err()
        );
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    #[test]
    fn panics_before_http_when_the_local_journal_is_missing() {
        let (dispatcher, _) = fixture();
        let client = Arc::new(StubClient::succeeds());

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                upload_local_journal(dispatcher.clone(), client.clone(), LocalJournalId::new(99))
            }))
            .is_err()
        );
        assert!(client.put_requests().is_empty());
        assert!(client.get_requests().is_empty());
    }

    fn issue(id: u16) -> IssueAggregate {
        sample_issue_aggregate(id, "subject", IssueStatusId::new(1), None, None, None, 0)
    }

    fn journal(id: u16, notes: &str) -> Journal {
        let mut journal = parse_journal_yaml(JournalId::new(id.min(3)));
        journal.id = JournalId::new(id);
        journal.notes = notes.to_string();
        journal
    }

    struct StubClient {
        get_result: Result<(IssueAggregate, Vec<Journal>), RedmineClientError>,
        put_result: Result<(), RedmineClientError>,
        gets: Mutex<Vec<IssueId>>,
        puts: Mutex<Vec<(IssueId, String)>>,
        calls: Mutex<Vec<String>>,
    }

    impl StubClient {
        fn succeeds() -> Self {
            Self::succeeds_with(issue(42), fetch())
        }
        fn succeeds_with(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self::new(Ok((issue, journals)), Ok(()))
        }
        fn fails_put() -> Self {
            Self::new(
                Ok((issue(42), Vec::new())),
                Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
            )
        }
        fn fails_get() -> Self {
            Self::new(
                Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
                Ok(()),
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
                calls: Mutex::new(vec![]),
            }
        }
        fn get_requests(&self) -> Vec<IssueId> {
            self.gets.lock().unwrap().clone()
        }
        fn put_requests(&self) -> Vec<(IssueId, String)> {
            self.puts.lock().unwrap().clone()
        }
        fn call_order(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl RedmineClient for StubClient {
        async fn get_issue(
            &self,
            id: IssueId,
        ) -> Result<(IssueAggregate, Vec<Journal>), RedmineClientError> {
            self.calls.lock().unwrap().push("get".to_string());
            self.gets.lock().unwrap().push(id);
            self.get_result.clone()
        }
        async fn create_journal(
            &self,
            issue_id: IssueId,
            notes: &str,
        ) -> Result<(), RedmineClientError> {
            self.calls.lock().unwrap().push("put".to_string());
            self.puts
                .lock()
                .unwrap()
                .push((issue_id, notes.to_string()));
            self.put_result.clone()
        }
        async fn update_journal_notes(
            &self,
            _: JournalId,
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
