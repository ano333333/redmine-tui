use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Action, Dispatcher, IssueAction, IssueFetchState};
use crate::vos::{EntityIdValue, IssueId};

/// Issue詳細の取得完了Actionを1件返すFuture。
pub type FetchIssueFuture = Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

/// 未取得、または取得失敗状態のIssueについて詳細取得を開始する。
///
/// 取得開始Actionは同期的にqueueへ追加する。
pub fn fetch_issue<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: IssueId,
) -> Option<FetchIssueFuture>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let can_start = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        store.try_get_issue_state(id).is_none()
            && matches!(
                store.try_get_issue_fetch_state(id),
                None | Some(IssueFetchState::FetchFailed { .. })
            )
    };
    if !can_start {
        return None;
    }

    dispatcher
        .borrow_mut()
        .dispatch(IssueAction::StartFetching { id });

    Some(Box::pin(async move {
        match client.get_issue(id).await {
            Ok(fetched) if fetched.aggregate.issue.id == id => vec![Action::IssueFetchSucceeded {
                id,
                issue: fetched.aggregate,
                children: fetched.children,
            }],
            // 応答IDの不一致はサーバー側の外部データ異常のため、Storeでpanicさせず取得失敗にする。
            Ok(fetched) => vec![
                IssueAction::FetchFailed {
                    id,
                    message: format!(
                        "requested issue {} but Redmine returned issue {}",
                        id.get(),
                        fetched.aggregate.issue.id.get()
                    ),
                }
                .into(),
            ],
            Err(error) => vec![
                IssueAction::FetchFailed {
                    id,
                    message: error.to_string(),
                }
                .into(),
            ],
        }
    }))
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
    use crate::stores::{Action, Dispatcher, IssueAction, IssueFetchState, IssueState};
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::{IssueId, IssueStatusId, JournalId};

    use super::fetch_issue;

    #[tokio::test]
    async fn dispatches_start_synchronously_without_consuming_it() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(42)));

        let future = fetch_issue(dispatcher.clone(), client.clone(), IssueId::new(42));

        assert!(future.is_some());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert_eq!(dispatcher.borrow().store().try_get_issue_state(42), None);
        assert_eq!(
            dispatcher.borrow().store().try_get_issue_fetch_state(42),
            None
        );
        assert_eq!(client.requested_ids(), Vec::<IssueId>::new());
    }

    #[tokio::test]
    async fn returns_one_completion_with_the_requested_issue_and_its_journals() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds_with_journals(
            issue(42),
            vec![journal(1, 42), journal(2, 42)],
        ));

        let actions = fetch_issue(dispatcher, client.clone(), IssueId::new(42))
            .expect("unregistered issue should start fetching")
            .await;

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::IssueFetchSucceeded { id, issue, .. } => {
                assert_eq!(*id, IssueId::new(42));
                assert_eq!(issue.issue.id, IssueId::new(42));
                let journal_ids: Vec<JournalId> = issue.journals.iter().map(|j| j.id).collect();
                assert_eq!(journal_ids, vec![JournalId::new(1), JournalId::new(2)]);
            }
            _ => panic!("successful request must return IssueFetchSucceeded"),
        }
        assert_eq!(client.requested_ids(), vec![IssueId::new(42)]);
    }

    #[tokio::test]
    async fn client_error_returns_fetch_failed() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::fails(RedmineClientError::Network {
            reason: "offline".to_string(),
        }));

        let actions = fetch_issue(dispatcher, client, IssueId::new(42))
            .expect("unregistered issue should start fetching")
            .await;

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::Issue(IssueAction::FetchFailed { id, message }) => {
                assert_eq!(*id, IssueId::new(42));
                assert_eq!(message, "network error: offline");
            }
            _ => panic!("client error must return FetchFailed"),
        }
    }

    #[tokio::test]
    async fn response_id_mismatch_returns_fetch_failed() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(99)));

        let actions = fetch_issue(dispatcher, client, IssueId::new(42))
            .expect("unregistered issue should start fetching")
            .await;

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::Issue(IssueAction::FetchFailed { id, message }) => {
                assert_eq!(*id, IssueId::new(42));
                assert!(message.contains("42"));
                assert!(message.contains("99"));
            }
            _ => panic!("mismatched response must return FetchFailed"),
        }
    }

    #[tokio::test]
    async fn retries_from_fetch_failed() {
        let dispatcher = dispatcher();
        dispatch_and_consume(&dispatcher, IssueAction::StartFetching { id: 42.into() });
        dispatch_and_consume(
            &dispatcher,
            IssueAction::FetchFailed {
                id: 42.into(),
                message: "first failure".to_string(),
            },
        );
        let client = Arc::new(StubClient::succeeds(issue(42)));

        let future = fetch_issue(dispatcher.clone(), client, IssueId::new(42));

        assert!(future.is_some());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert_eq!(
            dispatcher.borrow().store().try_get_issue_fetch_state(42),
            Some(IssueFetchState::FetchFailed {
                message: "first failure".to_string(),
            })
        );
    }

    #[test]
    fn suppresses_duplicate_after_start_action_is_consumed() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(42)));
        let first = fetch_issue(dispatcher.clone(), client.clone(), IssueId::new(42));
        assert!(first.is_some());
        dispatcher.borrow_mut().consume_action();

        let second = fetch_issue(dispatcher.clone(), client.clone(), IssueId::new(42));

        assert!(second.is_none());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(client.requested_ids(), Vec::<IssueId>::new());
    }

    #[test]
    fn suppresses_requests_for_every_loaded_state() {
        for state in [
            IssueState::Synced,
            IssueState::Edited,
            IssueState::Uploading,
        ] {
            let dispatcher = dispatcher_in_loaded_state(state.clone());
            let client = Arc::new(StubClient::succeeds(issue(42)));

            let future = fetch_issue(dispatcher.clone(), client.clone(), IssueId::new(42));

            assert!(future.is_none(), "state {state:?} must suppress fetch");
            assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
            assert_eq!(client.requested_ids(), Vec::<IssueId>::new());
        }
    }

    fn dispatcher() -> Rc<RefCell<Dispatcher>> {
        Rc::new(RefCell::new(Dispatcher::new()))
    }

    fn dispatch_and_consume(dispatcher: &Rc<RefCell<Dispatcher>>, action: IssueAction) {
        dispatcher.borrow_mut().dispatch(action);
        dispatcher.borrow_mut().consume_action();
    }

    fn dispatcher_in_loaded_state(state: IssueState) -> Rc<RefCell<Dispatcher>> {
        let dispatcher = dispatcher();
        crate::test_support::dispatch_loaded_issue(&mut dispatcher.borrow_mut(), issue(42));
        if matches!(state, IssueState::Edited | IssueState::Uploading) {
            dispatch_and_consume(
                &dispatcher,
                IssueAction::UpdateDescription {
                    id: 42.into(),
                    body: "edited".to_string(),
                },
            );
        }
        if state == IssueState::Uploading {
            dispatch_and_consume(&dispatcher, IssueAction::StartUpload { id: 42.into() });
        }
        dispatcher
    }

    fn issue(id: u16) -> IssueAggregate {
        sample_issue_aggregate(id, "subject", IssueStatusId::new(1), None, None, None, 0)
    }

    fn journal(id: u16, issue_id: u16) -> Journal {
        Journal {
            id: JournalId::new(id),
            issue_id: IssueId::new(issue_id),
            user: "alice".to_string(),
            updated_on: Some(local_datetime("2026-01-15T00:00:00+09:00")),
            details: vec![],
            notes: format!("fetched notes {id}"),
        }
    }

    struct StubClient {
        result: Result<IssueAggregate, RedmineClientError>,
        journals: Vec<Journal>,
        requested_ids: Mutex<Vec<IssueId>>,
    }

    impl StubClient {
        fn succeeds(issue: IssueAggregate) -> Self {
            Self::succeeds_with_journals(issue, vec![])
        }

        fn succeeds_with_journals(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self {
                result: Ok(issue),
                journals,
                requested_ids: Mutex::new(Vec::new()),
            }
        }

        fn fails(error: RedmineClientError) -> Self {
            Self {
                result: Err(error),
                requested_ids: Mutex::new(Vec::new()),
                journals: vec![],
            }
        }

        fn requested_ids(&self) -> Vec<IssueId> {
            self.requested_ids.lock().unwrap().clone()
        }
    }

    impl RedmineClient for StubClient {
        async fn get_issue(&self, id: IssueId) -> Result<FetchedIssue, RedmineClientError> {
            self.requested_ids.lock().unwrap().push(id);
            self.result.clone().map(|mut aggregate| {
                aggregate.journals = self.journals.clone();
                FetchedIssue {
                    aggregate,
                    children: vec![],
                }
            })
        }

        async fn update_issue(
            &self,
            _: crate::vos::IssueId,
            _: &crate::clients::redmine::IssueUpdate,
        ) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_issue_notes(
            &self,
            _: crate::vos::IssueId,
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
