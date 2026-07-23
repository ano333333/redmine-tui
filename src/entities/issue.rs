use chrono::{DateTime, Local};

use crate::vos::{
    IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

pub struct Issue {
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
    pub component: String,
    pub description: String,
    pub child_ids: Vec<IssueId>,
    pub journal_ids: Vec<u16>,
}
