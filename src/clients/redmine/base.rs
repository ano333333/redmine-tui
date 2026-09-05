use serde::Serialize;

use std::num::NonZeroUsize;

use crate::entities::{
    Category, IssueAggregate, IssueStatus, Priority, Project, ProjectIssuesPage, TargetVersion,
    TimeEntityActivity, Tracker, User,
};
use crate::vos::{IssueId, JournalId, ProjectId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedmineHttpError {
    pub method: String,
    pub url: String,
    pub status_code: u16,
    pub response_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum RedmineClientError {
    BadRequest { context: RedmineHttpError },
    Unauthorized { context: RedmineHttpError },
    // 403, 404
    NotFound { context: RedmineHttpError },
    UnprocessableEntity { context: RedmineHttpError },
    // 5xx
    InternalServerError { context: RedmineHttpError },
    Network { reason: String },
    Client { reason: String },
}

impl std::fmt::Display for RedmineClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest { context } => write!(f, "bad request: {context}"),
            Self::Unauthorized { context } => write!(f, "unauthorized: {context}"),
            Self::NotFound { context } => write!(f, "not found: {context}"),
            Self::UnprocessableEntity { context } => {
                write!(f, "unprocessable entity: {context}")
            }
            Self::InternalServerError { context } => {
                write!(f, "internal server error: {context}")
            }
            Self::Network { reason } => write!(f, "network error: {reason}"),
            Self::Client { reason } => write!(f, "client error: {reason}"),
        }
    }
}

impl std::error::Error for RedmineClientError {}

impl std::fmt::Display for RedmineHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} returned {} with body: {}",
            self.method, self.url, self.status_code, self.response_body
        )
    }
}

#[allow(async_fn_in_trait)]
pub trait RedmineClient {
    async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError>;
    fn get_issue(
        &self,
        id: IssueId,
    ) -> impl std::future::Future<Output = Result<IssueAggregate, RedmineClientError>> + Send;
    async fn update_issue(&self, issue: &IssueAggregate) -> Result<(), RedmineClientError>;
    async fn update_journal_notes(
        &self,
        id: JournalId,
        notes: &str,
    ) -> Result<(), RedmineClientError>;
    async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError>;
    async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError>;
    async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError>;
    fn get_project_issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> impl std::future::Future<Output = Result<ProjectIssuesPage, RedmineClientError>> + Send;
    async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError>;
    async fn get_time_entity_activities(
        &self,
    ) -> Result<Vec<TimeEntityActivity>, RedmineClientError>;
    async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError>;
    async fn get_users(&self) -> Result<Vec<User>, RedmineClientError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>(_: T) {}

    fn assert_project_issues_future_is_send<C: RedmineClient>(client: &C) {
        assert_send(client.get_project_issues(ProjectId::new(1), NonZeroUsize::new(1).unwrap()));
    }

    #[test]
    fn project_issues_future_is_send() {
        let client =
            crate::clients::redmine::DefaultRedmineClient::new("http://example.test", "token");

        assert_project_issues_future_is_send(&client);
    }
}
