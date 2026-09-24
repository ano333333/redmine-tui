use serde::Serialize;

use std::num::NonZeroUsize;

use crate::entities::{
    Category, IssueAggregate, IssueStatus, Journal, Priority, Project, ProjectIssuesPage,
    TargetVersion, TimeEntityActivity, Tracker, User,
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

/// RedmineのIssue詳細取得結果を、Issue本体とJournal一覧に分けて受け渡すための型。
///
/// 永続的なdomain entityである[`IssueAggregate`]とは異なり、Client境界で一度の
/// レスポンスから変換した複数の保存単位をまとめて返すためだけに使用する。
pub struct FetchedIssue {
    pub aggregate: IssueAggregate,
    pub journals: Vec<Journal>,
}

/// Redmineとの通信をplatform固有の実装から分離する境界。
///
/// 各メソッドのFutureはbackground taskとして実行するため`Send`を契約とする。
/// 実装は、awaitをまたいで非`Send`な値を保持してはならない。
pub trait RedmineClient {
    fn get_categories(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Category>, RedmineClientError>> + Send;
    fn get_issue(
        &self,
        id: IssueId,
    ) -> impl std::future::Future<Output = Result<FetchedIssue, RedmineClientError>> + Send;
    fn update_issue(
        &self,
        issue: &IssueAggregate,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send;
    /// Redmine上の既存Journalのnotes全体を指定値で置き換える。
    fn update_journal_notes(
        &self,
        journal_id: JournalId,
        notes: &str,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send;
    /// Issueのnotesだけを更新対象としてRedmineへ送り、新しいJournalを作成する。
    ///
    /// [`Self::update_issue`]とは異なり、Issueの未保存propertyは送信しない。
    fn update_issue_notes(
        &self,
        issue_id: IssueId,
        notes: &str,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send;
    fn get_issue_statuses(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<IssueStatus>, RedmineClientError>> + Send;
    fn get_priorities(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Priority>, RedmineClientError>> + Send;
    fn get_projects(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Project>, RedmineClientError>> + Send;
    fn get_project_issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> impl std::future::Future<Output = Result<ProjectIssuesPage, RedmineClientError>> + Send;
    fn get_target_versions(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TargetVersion>, RedmineClientError>> + Send;
    fn get_time_entity_activities(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TimeEntityActivity>, RedmineClientError>> + Send;
    fn get_trackers(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Tracker>, RedmineClientError>> + Send;
    fn get_users(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<User>, RedmineClientError>> + Send;
}
