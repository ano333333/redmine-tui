use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Action, Dispatcher, IssueAction, IssueState};
use crate::vos::{EntityIdValue, IssueId};

pub type FetchIssueFuture = Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

/// 未取得、または取得失敗状態のIssueについて詳細取得を開始する。
///
/// 取得開始Actionは同期的にqueueへ追加する。返却したFutureは、その後ろへ順番に
/// 追加するHTTP完了Action列を返す。
pub fn fetch_issue<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: IssueId,
) -> Option<FetchIssueFuture>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let can_start = matches!(
        dispatcher.borrow().store().get_issue_state(id),
        None | Some(IssueState::FetchFailed { .. })
    );
    if !can_start {
        return None;
    }

    dispatcher
        .borrow_mut()
        .dispatch(IssueAction::StartFetching { id });

    Some(Box::pin(async move {
        match client.get_issue(id).await {
            Ok((issue, journals)) if issue.issue.id == id => {
                super::initial_issue_details_actions(issue, journals)
            }
            Ok((issue, _)) => vec![
                IssueAction::FetchFailed {
                    id,
                    message: format!(
                        "requested issue {} but Redmine returned issue {}",
                        id.get(),
                        issue.issue.id.get()
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

    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{Action, Dispatcher, IssueAction, IssueState, JournalAction};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId, JournalKey};

    use super::fetch_issue;

    #[tokio::test]
    async fn queues_start_before_starting_the_http_request() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(42)));

        let future = fetch_issue(dispatcher.clone(), client.clone(), IssueId::new(42));

        assert!(future.is_some());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert_eq!(client.requested_ids(), Vec::<IssueId>::new());
    }

    #[tokio::test]
    async fn returns_issue_and_journals_after_getting_the_requested_issue() {
        let dispatcher = dispatcher();
        let mut fetched_issue = issue(42);
        fetched_issue.journal_keys = vec![
            JournalKey::Remote(JournalId::new(2)),
            JournalKey::Remote(JournalId::new(1)),
        ];
        let client = Arc::new(StubClient::succeeds_with_journals(
            fetched_issue,
            vec![
                parse_journal_yaml(JournalId::new(2)),
                parse_journal_yaml(JournalId::new(1)),
            ],
        ));

        let actions = fetch_issue(dispatcher, client.clone(), IssueId::new(42))
            .expect("unregistered issue should start fetching")
            .await;

        let [
            Action::Journal(JournalAction::RegisterRemote {
                journal: first,
                issue_id: first_owner,
            }),
            Action::Journal(JournalAction::RegisterRemote {
                journal: second,
                issue_id: second_owner,
            }),
            Action::Issue(IssueAction::FetchSucceeded { id, issue }),
        ] = actions.as_slice()
        else {
            panic!("successful request must return journals then the issue")
        };
        assert_eq!(
            (*first_owner, *second_owner, *id),
            (42.into(), 42.into(), 42.into())
        );
        assert_eq!(
            (first.id, second.id),
            (JournalId::new(2), JournalId::new(1))
        );
        assert_eq!(issue.issue.id, IssueId::new(42));
        assert_eq!(client.requested_ids(), vec![IssueId::new(42)]);
    }

    #[tokio::test]
    async fn converts_client_error_to_fetch_failed() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::fails(RedmineClientError::Network {
            reason: "offline".to_string(),
        }));

        let actions = fetch_issue(dispatcher, client, IssueId::new(42))
            .expect("unregistered issue should start fetching")
            .await;

        match actions.as_slice() {
            [Action::Issue(IssueAction::FetchFailed { id, message })] => {
                assert_eq!(*id, IssueId::new(42));
                assert_eq!(message, "network error: offline");
            }
            _ => panic!("client error must return FetchFailed"),
        }
    }

    #[tokio::test]
    async fn converts_response_id_mismatch_to_fetch_failed() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(issue(99)));

        let actions = fetch_issue(dispatcher, client, IssueId::new(42))
            .expect("unregistered issue should start fetching")
            .await;

        match actions.as_slice() {
            [Action::Issue(IssueAction::FetchFailed { id, message })] => {
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
            dispatcher.borrow().store().get_issue_state(42),
            Some(&IssueState::FetchFailed {
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
        dispatch_and_consume(&dispatcher, IssueAction::Sync { issue: issue(42) });
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

    struct StubClient {
        result: Result<(IssueAggregate, Vec<crate::entities::Journal>), RedmineClientError>,
        requested_ids: Mutex<Vec<IssueId>>,
    }

    impl StubClient {
        fn succeeds(issue: IssueAggregate) -> Self {
            Self::succeeds_with_journals(issue, Vec::new())
        }

        fn succeeds_with_journals(
            issue: IssueAggregate,
            journals: Vec<crate::entities::Journal>,
        ) -> Self {
            Self {
                result: Ok((issue, journals)),
                requested_ids: Mutex::new(Vec::new()),
            }
        }

        fn fails(error: RedmineClientError) -> Self {
            Self {
                result: Err(error),
                requested_ids: Mutex::new(Vec::new()),
            }
        }

        fn requested_ids(&self) -> Vec<IssueId> {
            self.requested_ids.lock().unwrap().clone()
        }
    }

    impl RedmineClient for StubClient {
        async fn get_issue(
            &self,
            id: IssueId,
        ) -> Result<(IssueAggregate, Vec<crate::entities::Journal>), RedmineClientError> {
            self.requested_ids.lock().unwrap().push(id);
            self.result.clone()
        }

        async fn update_issue(&self, _: &IssueAggregate) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
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
