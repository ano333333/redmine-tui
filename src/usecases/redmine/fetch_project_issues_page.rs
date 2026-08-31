use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, ProjectIssuesAction, ProjectIssuesRequestId};
use crate::vos::ProjectId;

pub type FetchProjectIssuesPageFuture =
    Pin<Box<dyn Future<Output = ProjectIssuesAction> + Send + 'static>>;

/// プロジェクト別Issueページの取得を、新しいrequest IDで開始する。
///
/// StartLoadingは同期的にqueueへ追加し、返却したFutureは同じrequest IDを持つ
/// LoadSucceededまたはLoadFailedを返す。
pub fn fetch_project_issues_page<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    project_id: ProjectId,
    page: NonZeroUsize,
) -> FetchProjectIssuesPageFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let request_id = ProjectIssuesRequestId::new();
    dispatcher
        .borrow_mut()
        .dispatch(ProjectIssuesAction::StartLoading {
            request_id,
            project_id,
            page,
        });

    Box::pin(async move {
        match client.get_project_issues(project_id, page).await {
            Ok(result) => ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id,
                page,
                result,
            },
            Err(error) => ProjectIssuesAction::LoadFailed {
                request_id,
                project_id,
                page,
                message: error.to_string(),
            },
        }
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::num::NonZeroUsize;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, Issue, IssueAggregate, IssueStatus, Priority, Project, ProjectIssuesPage,
        TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{Dispatcher, ProjectIssuesAction, ProjectIssuesRequestId};
    use crate::vos::{IssueId, IssueStatusId, ProjectId};

    use super::fetch_project_issues_page;

    const PROJECT_ID: ProjectId = ProjectId::new(12);

    #[test]
    fn queues_start_loading_without_consuming_or_starting_http() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(page_result(0, 0)));

        let _future =
            fetch_project_issues_page(dispatcher.clone(), client.clone(), PROJECT_ID, page(1));

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(PROJECT_ID, page(1))
                .is_none()
        );
        assert!(client.requested_pages().is_empty());
    }

    #[tokio::test]
    async fn each_call_uses_a_fresh_request_id_for_its_start_and_completion() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(page_result(1, 0)));

        let first =
            fetch_project_issues_page(dispatcher.clone(), client.clone(), PROJECT_ID, page(1));
        let first_start_id = consume_start_id(&dispatcher);
        let second = fetch_project_issues_page(dispatcher.clone(), client, PROJECT_ID, page(1));
        let second_start_id = consume_start_id(&dispatcher);

        assert_ne!(first_start_id, second_start_id);
        assert_eq!(completion_id(first.await), first_start_id);
        assert_eq!(completion_id(second.await), second_start_id);
    }

    #[tokio::test]
    async fn loading_exact_key_can_start_a_fresh_request() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(page_result(1, 0)));

        let _first =
            fetch_project_issues_page(dispatcher.clone(), client.clone(), PROJECT_ID, page(1));
        let first_request_id = consume_start_id(&dispatcher);

        let _second = fetch_project_issues_page(dispatcher.clone(), client, PROJECT_ID, page(1));
        let second_request_id = consume_start_id(&dispatcher);

        assert_ne!(second_request_id, first_request_id);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(PROJECT_ID, page(1)),
            Some(&crate::stores::ProjectIssuesPageState::Loading {
                request_id: second_request_id,
            })
        );
    }

    #[tokio::test]
    async fn loaded_exact_key_can_start_a_fresh_request() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(page_result(1, 0)));
        let first =
            fetch_project_issues_page(dispatcher.clone(), client.clone(), PROJECT_ID, page(1));
        let first_request_id = consume_start_id(&dispatcher);
        dispatcher.borrow_mut().dispatch(first.await);
        dispatcher.borrow_mut().consume_action();

        let _second = fetch_project_issues_page(dispatcher.clone(), client, PROJECT_ID, page(1));
        let second_request_id = consume_start_id(&dispatcher);

        assert_ne!(second_request_id, first_request_id);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(PROJECT_ID, page(1)),
            Some(&crate::stores::ProjectIssuesPageState::Loading {
                request_id: second_request_id,
            })
        );
    }

    #[tokio::test]
    async fn failed_exact_key_can_start_a_fresh_request() {
        let dispatcher = dispatcher();
        let failing_client = Arc::new(StubClient::fails(RedmineClientError::Network {
            reason: "offline".to_string(),
        }));
        let first =
            fetch_project_issues_page(dispatcher.clone(), failing_client, PROJECT_ID, page(1));
        let first_request_id = consume_start_id(&dispatcher);
        dispatcher.borrow_mut().dispatch(first.await);
        dispatcher.borrow_mut().consume_action();

        let _second = fetch_project_issues_page(
            dispatcher.clone(),
            Arc::new(StubClient::succeeds(page_result(1, 0))),
            PROJECT_ID,
            page(1),
        );
        let second_request_id = consume_start_id(&dispatcher);

        assert_ne!(second_request_id, first_request_id);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(PROJECT_ID, page(1)),
            Some(&crate::stores::ProjectIssuesPageState::Loading {
                request_id: second_request_id,
            })
        );
    }

    #[tokio::test]
    async fn maps_a_successful_http_response_to_load_succeeded() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::succeeds(page_result(1, 0)));

        let action =
            fetch_project_issues_page(dispatcher.clone(), client.clone(), PROJECT_ID, page(1))
                .await;
        let start_id = consume_start_id(&dispatcher);

        let ProjectIssuesAction::LoadSucceeded {
            request_id,
            project_id,
            page: actual_page,
            result,
        } = action
        else {
            panic!("successful HTTP response must produce LoadSucceeded");
        };
        assert_eq!(request_id, start_id);
        assert_eq!(project_id, PROJECT_ID);
        assert_eq!(actual_page, page(1));
        assert_eq!(result.total_count, 1);
        assert_eq!(client.requested_pages(), vec![(PROJECT_ID, page(1))]);
    }

    #[tokio::test]
    async fn maps_an_http_error_to_displayable_load_failed() {
        let dispatcher = dispatcher();
        let client = Arc::new(StubClient::fails(RedmineClientError::Network {
            reason: "offline".to_string(),
        }));

        let action =
            fetch_project_issues_page(dispatcher.clone(), client, PROJECT_ID, page(1)).await;
        let start_id = consume_start_id(&dispatcher);

        let ProjectIssuesAction::LoadFailed {
            request_id,
            project_id,
            page: actual_page,
            message,
        } = action
        else {
            panic!("HTTP error must produce LoadFailed");
        };
        assert_eq!(request_id, start_id);
        assert_eq!(project_id, PROJECT_ID);
        assert_eq!(actual_page, page(1));
        assert_eq!(message, "network error: offline");
    }

    fn consume_start_id(dispatcher: &Rc<RefCell<Dispatcher>>) -> ProjectIssuesRequestId {
        dispatcher.borrow_mut().consume_action();
        let dispatcher = dispatcher.borrow();
        let Some(crate::stores::ProjectIssuesPageState::Loading { request_id }) = dispatcher
            .store()
            .get_project_issues_page_state(PROJECT_ID, page(1))
        else {
            panic!("StartLoading must install Loading");
        };
        *request_id
    }

    fn completion_id(action: ProjectIssuesAction) -> ProjectIssuesRequestId {
        match action {
            ProjectIssuesAction::LoadSucceeded { request_id, .. }
            | ProjectIssuesAction::LoadFailed { request_id, .. } => request_id,
            ProjectIssuesAction::StartLoading { .. } => panic!("future must return completion"),
        }
    }

    fn dispatcher() -> Rc<RefCell<Dispatcher>> {
        Rc::new(RefCell::new(Dispatcher::new()))
    }

    fn page(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    fn page_result(total_count: usize, offset: usize) -> ProjectIssuesPage {
        ProjectIssuesPage {
            issues: (total_count > 0)
                .then(|| Issue {
                    id: IssueId::new(1),
                    project_id: PROJECT_ID,
                    subject: "listed issue".to_string(),
                    description: "description".to_string(),
                    status_id: IssueStatusId::new(1),
                })
                .into_iter()
                .collect(),
            total_count,
            offset,
            limit: 50,
        }
    }

    struct StubClient {
        result: Result<ProjectIssuesPage, RedmineClientError>,
        requested_pages: Mutex<Vec<(ProjectId, NonZeroUsize)>>,
    }

    impl StubClient {
        fn succeeds(result: ProjectIssuesPage) -> Self {
            Self {
                result: Ok(result),
                requested_pages: Mutex::new(Vec::new()),
            }
        }

        fn fails(error: RedmineClientError) -> Self {
            Self {
                result: Err(error),
                requested_pages: Mutex::new(Vec::new()),
            }
        }

        fn requested_pages(&self) -> Vec<(ProjectId, NonZeroUsize)> {
            self.requested_pages.lock().unwrap().clone()
        }
    }

    impl RedmineClient for StubClient {
        async fn get_project_issues(
            &self,
            project_id: ProjectId,
            page: NonZeroUsize,
        ) -> Result<ProjectIssuesPage, RedmineClientError> {
            self.requested_pages
                .lock()
                .unwrap()
                .push((project_id, page));
            self.result.clone()
        }

        async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
            unreachable!()
        }
        async fn get_issue(&self, _: IssueId) -> Result<IssueAggregate, RedmineClientError> {
            unreachable!()
        }
        async fn update_issue(&self, _: &IssueAggregate) -> Result<(), RedmineClientError> {
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
