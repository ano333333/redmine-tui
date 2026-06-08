use chrono::{DateTime, Local};

#[derive(Clone)]
pub struct Journal {
    pub id: u16,
    pub creator: String,
    pub updated_at: DateTime<Local>,
    pub properties: Vec<JournalPropertyChange>,
    pub comment: Option<String>,
}

#[derive(Clone)]
pub struct JournalPropertyChange {
    pub target: String,
    pub old: String,
    pub new: String,
}
