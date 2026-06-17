use chrono::{DateTime, Local};

pub struct Issue {
    pub id: u16,
    pub subject: String,
    pub author: String,
    pub created_on: DateTime<Local>,
    pub updated_on: DateTime<Local>,
    pub status_id: u16,
    pub priority: String,
    pub assigned_to: Option<String>,
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
