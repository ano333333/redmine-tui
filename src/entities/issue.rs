use chrono::{DateTime, Local};

use super::{IssueId, IssueStatusId, PriorityId, TrackerId, UserId};

pub struct Issue {
    pub id: IssueId,
    pub subject: String,
    pub author_id: UserId,
    pub created_on: DateTime<Local>,
    pub updated_on: DateTime<Local>,
    pub tracker_id: TrackerId,
    pub status_id: IssueStatusId,
    pub priority_id: PriorityId,
    pub assigned_to_id: Option<UserId>,
    pub fixed_version: Option<String>,
    pub start_date: Option<DateTime<Local>>,
    pub due_date: Option<DateTime<Local>>,
    pub done_ratio: u16,
    pub estimated_hours: Option<u16>,
    pub resolve_way: Option<String>,
    pub component: String,
    pub tags: Vec<String>,
    pub description: String,
    pub child_ids: Vec<IssueId>,
    pub journal_ids: Vec<u16>,
}
