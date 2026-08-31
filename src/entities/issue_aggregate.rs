use chrono::{DateTime, Local};

use crate::entities::Issue;
use crate::vos::{CategoryId, IssueId, JournalKey, PriorityId, TargetVersionId, TrackerId, UserId};

#[derive(Debug, Clone)]
pub struct IssueAggregate {
    pub issue: Issue,
    pub author_id: UserId,
    pub created_on: DateTime<Local>,
    pub updated_on: DateTime<Local>,
    pub tracker_id: TrackerId,
    pub priority_id: PriorityId,
    pub assigned_to_id: Option<UserId>,
    pub target_version_id: Option<TargetVersionId>,
    pub start_date: Option<DateTime<Local>>,
    pub due_date: Option<DateTime<Local>>,
    pub done_ratio: u16,
    pub estimated_hours: Option<u16>,
    pub total_spent_hours: Option<f64>,
    pub category_id: Option<CategoryId>,
    pub child_ids: Vec<IssueId>,
    pub journal_keys: Vec<JournalKey>,
}
