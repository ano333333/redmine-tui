use serde::Serialize;

use crate::entities::{
    Category, Issue, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity, Tracker,
    User,
};
use crate::vos::IssueId;

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
    async fn get_issue(&self, id: IssueId) -> Result<Issue, RedmineClientError>;
    async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError>;
    async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError>;
    async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError>;
    async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError>;
    async fn get_time_entity_activities(
        &self,
    ) -> Result<Vec<TimeEntityActivity>, RedmineClientError>;
    async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError>;
    async fn get_users(&self) -> Result<Vec<User>, RedmineClientError>;
}
