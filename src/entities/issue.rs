use chrono::{DateTime, Local};

use super::IssueId;

pub struct Issue {
    pub id: IssueId,
    pub subject: String,
    pub author_id: u16,
    pub created_on: DateTime<Local>,
    pub updated_on: DateTime<Local>,
    pub status_id: u16,
    pub priority_id: u16,
    pub assigned_to_id: Option<u16>,
    pub fixed_version: Option<String>,
    pub start_date: Option<DateTime<Local>>,
    pub due_date: Option<DateTime<Local>>,
    pub done_ratio: u16,
    pub estimated_hours: Option<u16>,
    pub resolve_way: Option<String>,
    pub component: String,
    pub tags: Vec<String>,
    pub description: String,
    pub child_ids: Vec<u16>,
    pub journal_ids: Vec<u16>,
}
