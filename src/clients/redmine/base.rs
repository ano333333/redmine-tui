use serde::Serialize;

use std::num::NonZeroUsize;

use crate::entities::{
    Category, IssueAggregate, IssueChild, IssueStatus, Priority, Project, ProjectIssuesPage,
    TargetVersion, TimeEntityActivity, Tracker, User,
};
use chrono::{DateTime, Local};

use crate::vos::issue_property_diff::fold_property_diffs;
use crate::vos::{
    CategoryId, IssueId, IssuePropertyDiff, IssueStatusId, JournalId, PriorityId, ProjectId,
    TargetVersionId, TrackerId, UserId,
};

pub(crate) const PROJECT_ISSUES_PAGE_LIMIT: usize = 50;

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

/// RedmineのIssue詳細取得結果。
///
/// 永続的なdomain entityである[`IssueAggregate`]とは異なり、Client境界で一度の
/// レスポンスから変換した取得情報をまとめて返すためだけに使用する。
pub struct FetchedIssue {
    pub aggregate: IssueAggregate,
    /// 子Issueの一覧。子Issue自身の詳細は含まない。
    pub children: Vec<IssueChild>,
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
    /// `update`が持つIssue属性だけを送信し、送信しない属性のサーバー値は変えない。
    fn update_issue(
        &self,
        issue_id: IssueId,
        update: &IssueUpdate,
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

/// Issue属性の更新要求。編集した属性だけを持つ。
///
/// 外側の`None`は送信しないことを表し、現在のサーバー値を変えない。値を持てる属性の
/// `Some(None)`は、その属性の値を解除することを表す。APIでの解除の表現はClientが変換する。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssueUpdate {
    pub subject: Option<String>,
    pub description: Option<String>,
    pub project_id: Option<ProjectId>,
    pub tracker_id: Option<TrackerId>,
    pub status_id: Option<IssueStatusId>,
    pub priority_id: Option<PriorityId>,
    pub assigned_to_id: Option<Option<UserId>>,
    pub target_version_id: Option<Option<TargetVersionId>>,
    pub start_date: Option<Option<DateTime<Local>>>,
    pub due_date: Option<Option<DateTime<Local>>>,
    pub done_ratio: Option<u16>,
    pub estimated_hours: Option<Option<f64>>,
    pub category_id: Option<Option<CategoryId>>,
}

impl IssueUpdate {
    /// 属性ごとに集約したdiffの`after`を送信する更新要求を作る。
    ///
    /// 元の値に戻して差分がなくなった属性は送信しない。
    pub fn from_diffs(diffs: &[IssuePropertyDiff]) -> Self {
        let mut update = Self::default();
        for diff in fold_property_diffs(diffs) {
            match diff {
                IssuePropertyDiff::Subject(diff) => update.subject = Some(diff.after),
                IssuePropertyDiff::Description(diff) => update.description = Some(diff.after),
                IssuePropertyDiff::ProjectId(diff) => update.project_id = Some(diff.after),
                IssuePropertyDiff::TrackerId(diff) => update.tracker_id = Some(diff.after),
                IssuePropertyDiff::StatusId(diff) => update.status_id = Some(diff.after),
                IssuePropertyDiff::PriorityId(diff) => update.priority_id = Some(diff.after),
                IssuePropertyDiff::AssignedToId(diff) => update.assigned_to_id = Some(diff.after),
                IssuePropertyDiff::TargetVersionId(diff) => {
                    update.target_version_id = Some(diff.after)
                }
                IssuePropertyDiff::StartDate(diff) => update.start_date = Some(diff.after),
                IssuePropertyDiff::DueDate(diff) => update.due_date = Some(diff.after),
                IssuePropertyDiff::DoneRatio(diff) => update.done_ratio = Some(diff.after),
                IssuePropertyDiff::EstimatedHours(diff) => {
                    update.estimated_hours = Some(diff.after)
                }
                IssuePropertyDiff::CategoryId(diff) => update.category_id = Some(diff.after),
            }
        }
        update
    }
}

#[cfg(test)]
mod tests {
    use super::IssueUpdate;
    use crate::vos::issue_property_diff::{
        IssueAssignedToIdDiff, IssueDescriptionDiff, IssueStatusIdDiff,
    };
    use crate::vos::{IssuePropertyDiff, IssueStatusId, UserId};

    fn description_diff(before: &str, after: &str) -> IssuePropertyDiff {
        IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: before.to_string(),
            after: after.to_string(),
        })
    }

    #[test]
    fn sends_only_edited_properties_with_their_last_after() {
        let diffs = vec![
            description_diff("fetched", "first"),
            description_diff("first", "second"),
            IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: IssueStatusId::new(1),
                after: IssueStatusId::new(2),
            }),
        ];

        let update = IssueUpdate::from_diffs(&diffs);

        assert_eq!(
            update,
            IssueUpdate {
                description: Some("second".to_string()),
                status_id: Some(IssueStatusId::new(2)),
                ..IssueUpdate::default()
            }
        );
    }

    #[test]
    fn distinguishes_clearing_a_value_from_not_sending_it() {
        let diffs = vec![IssuePropertyDiff::AssignedToId(IssueAssignedToIdDiff {
            before: Some(UserId::new(7)),
            after: None,
        })];

        let update = IssueUpdate::from_diffs(&diffs);

        assert_eq!(update.assigned_to_id, Some(None));
        assert_eq!(update.category_id, None);
    }

    #[test]
    fn does_not_send_a_property_edited_back_to_its_original_value() {
        let diffs = vec![
            description_diff("fetched", "edited"),
            description_diff("edited", "fetched"),
        ];

        assert_eq!(IssueUpdate::from_diffs(&diffs), IssueUpdate::default());
    }
}
