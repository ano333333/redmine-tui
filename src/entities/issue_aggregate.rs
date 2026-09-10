use chrono::{DateTime, Local};

use crate::entities::Issue;
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, JournalId, PriorityId, ProjectId, TargetVersionId,
    TrackerId, UserId,
};

/// 詳細用のIssueエンティティ。
#[derive(Debug, Clone)]
pub struct IssueAggregate {
    /// 段階的な移行中のため、直下に残る一覧用フィールドと同じ値を保持する軽量なIssue。
    /// Step 1.17で直下の重複フィールドが削除されるまでの一時的な状態。
    pub issue: Issue,
    pub id: IssueId,
    pub subject: String,
    pub author_id: UserId,
    pub created_on: DateTime<Local>,
    pub updated_on: DateTime<Local>,
    pub project_id: ProjectId,
    pub tracker_id: TrackerId,
    pub status_id: IssueStatusId,
    pub priority_id: PriorityId,
    pub assigned_to_id: Option<UserId>,
    pub target_version_id: Option<TargetVersionId>,
    pub start_date: Option<DateTime<Local>>,
    pub due_date: Option<DateTime<Local>>,
    pub done_ratio: u16,
    pub estimated_hours: Option<u16>,
    pub total_spent_hours: Option<f64>,
    pub category_id: Option<CategoryId>,
    pub description: String,
    pub child_ids: Vec<IssueId>,
    pub journal_ids: Vec<JournalId>,
}
