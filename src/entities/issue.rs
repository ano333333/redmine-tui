use chrono::{DateTime, Local};

pub struct Issue {
    pub id: u16,
    pub title: String,
    pub creator: String,
    pub appended_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub status: String,
    pub priority: String,
    pub person_in_charge: Option<String>,
    pub target_version: Option<String>,
    pub start_date: Option<DateTime<Local>>,
    pub due: Option<DateTime<Local>>,
    pub progress: u16,
    pub planned_hours: Option<u16>,
    pub resolve_way: Option<String>,
    pub component: String,
    pub tags: Vec<String>,
    pub body: String,
    pub child_ids: Vec<u16>,
    pub journal_ids: Vec<u16>,
}

impl Issue {
    pub fn is_completed(&self) -> bool {
        self.status == "完了(closed)"
    }
}
