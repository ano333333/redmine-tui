use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use crate::clients::redmine::base::{FetchedIssue, PROJECT_ISSUES_PAGE_LIMIT};
use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::entities::{
    Category, IssueAggregate, IssueStatus, Priority, Project, ProjectIssuesPage, TargetVersion,
    TimeEntityActivity, Tracker, User,
};
use crate::vos::{IssueId, JournalId, ProjectId};

use super::fixture::DemoFixtureState;

#[derive(Clone)]
pub struct DemoRedmineClient {
    state: Arc<Mutex<DemoFixtureState>>,
}

impl DemoRedmineClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DemoFixtureState::initial())),
        }
    }

    fn read<T: Clone>(&self, select: fn(&DemoFixtureState) -> &Vec<T>) -> Vec<T> {
        // Futureを返す前に値を複製し、非同期処理中にlockを保持しない。
        let state = self.state.lock().expect("demo fixture state lock poisoned");
        select(&state).clone()
    }
}

impl Default for DemoRedmineClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RedmineClient for DemoRedmineClient {
    fn get_categories(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Category>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.categories);
        async move { Ok(value) }
    }

    fn get_issue(
        &self,
        _id: IssueId,
    ) -> impl std::future::Future<Output = Result<FetchedIssue, RedmineClientError>> + Send {
        async { todo!() }
    }

    fn update_issue(
        &self,
        _issue: &IssueAggregate,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
        async { todo!() }
    }

    fn update_journal_notes(
        &self,
        _journal_id: JournalId,
        _notes: &str,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
        async { todo!() }
    }

    fn update_issue_notes(
        &self,
        _issue_id: IssueId,
        _notes: &str,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
        async { todo!() }
    }

    fn get_issue_statuses(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<IssueStatus>, RedmineClientError>> + Send
    {
        let value = self.read(|state| &state.issue_statuses);
        async move { Ok(value) }
    }

    fn get_priorities(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Priority>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.priorities);
        async move { Ok(value) }
    }

    fn get_projects(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Project>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.projects);
        async move { Ok(value) }
    }

    fn get_project_issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> impl std::future::Future<Output = Result<ProjectIssuesPage, RedmineClientError>> + Send
    {
        let offset = page
            .get()
            .checked_sub(1)
            .and_then(|value| value.checked_mul(PROJECT_ISSUES_PAGE_LIMIT))
            .ok_or_else(|| RedmineClientError::Client {
                reason: format!("project issue page {} has an invalid offset", page.get()),
            });
        let result = offset.map(|offset| {
            let mut issues: Vec<_> = {
                let state = self.state.lock().expect("demo fixture state lock poisoned");
                state
                    .issues
                    .values()
                    .filter(|aggregate| aggregate.issue.project_id == project_id)
                    .map(|aggregate| aggregate.issue.clone())
                    .collect()
            };
            issues.sort_by_key(|issue| std::cmp::Reverse(issue.id));
            let total_count = issues.len();
            let issues = issues
                .into_iter()
                .skip(offset)
                .take(PROJECT_ISSUES_PAGE_LIMIT)
                .collect();
            ProjectIssuesPage {
                issues,
                total_count,
                offset,
                limit: PROJECT_ISSUES_PAGE_LIMIT,
            }
        });
        async move { result }
    }

    fn get_target_versions(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TargetVersion>, RedmineClientError>> + Send
    {
        let value = self.read(|state| &state.target_versions);
        async move { Ok(value) }
    }

    fn get_time_entity_activities(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TimeEntityActivity>, RedmineClientError>> + Send
    {
        let value = self.read(|state| &state.time_entity_activities);
        async move { Ok(value) }
    }

    fn get_trackers(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Tracker>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.trackers);
        async move { Ok(value) }
    }

    fn get_users(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<User>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.users);
        async move { Ok(value) }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use crate::clients::redmine::RedmineClient;
    use crate::clients::redmine::demo::DemoRedmineClient;
    use crate::vos::{EntityIdValue, ProjectId};
    use tokio::runtime::Builder;

    #[test]
    fn project_issue_pages_filter_sort_and_report_metadata() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();
        let first_page = runtime
            .block_on(client.get_project_issues(ProjectId::new(1), NonZeroUsize::new(1).unwrap()))
            .unwrap();
        assert_eq!(
            first_page
                .issues
                .iter()
                .map(|issue| issue.id.get())
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert!(
            first_page
                .issues
                .iter()
                .all(|issue| issue.project_id == ProjectId::new(1))
        );
        assert_eq!(first_page.total_count, 3);
        assert_eq!(first_page.offset, 0);
        assert_eq!(first_page.limit, 50);

        let other_project = runtime
            .block_on(client.get_project_issues(ProjectId::new(2), NonZeroUsize::new(1).unwrap()))
            .unwrap();
        assert!(other_project.issues.is_empty());
        assert_eq!(other_project.total_count, 0);

        let outside_range = runtime
            .block_on(client.get_project_issues(ProjectId::new(1), NonZeroUsize::new(2).unwrap()))
            .unwrap();
        assert!(outside_range.issues.is_empty());
        assert_eq!(outside_range.total_count, 3);
        assert_eq!(outside_range.offset, 50);
    }

    #[test]
    fn master_getters_return_embedded_fixture_values() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();

        macro_rules! names {
            ($values:expr) => {
                $values
                    .into_iter()
                    .map(|value| value.name)
                    .collect::<Vec<_>>()
            };
        }

        let categories = names!(runtime.block_on(client.get_categories()).unwrap());
        assert_eq!(categories.len(), 1);
        assert!(categories.contains(&String::from("category1")));

        let statuses = names!(runtime.block_on(client.get_issue_statuses()).unwrap());
        assert_eq!(statuses.len(), 6);
        assert!(statuses.contains(&String::from("新規(new)")));

        let priorities = names!(runtime.block_on(client.get_priorities()).unwrap());
        assert_eq!(priorities.len(), 4);
        assert!(priorities.contains(&String::from("major")));

        let projects = names!(runtime.block_on(client.get_projects()).unwrap());
        assert_eq!(projects.len(), 2);
        assert!(projects.contains(&String::from("Sample Project")));

        let versions = names!(runtime.block_on(client.get_target_versions()).unwrap());
        assert_eq!(versions.len(), 1);
        assert!(versions.contains(&String::from("v1.2.3")));

        let activities = names!(
            runtime
                .block_on(client.get_time_entity_activities())
                .unwrap()
        );
        assert_eq!(activities.len(), 3);
        assert!(activities.contains(&String::from("設計")));

        let trackers = names!(runtime.block_on(client.get_trackers()).unwrap());
        assert_eq!(trackers.len(), 3);
        assert!(trackers.contains(&String::from("Bug")));

        let users = names!(runtime.block_on(client.get_users()).unwrap());
        assert_eq!(users.len(), 2);
        assert!(users.contains(&String::from("user1")));
    }
}
