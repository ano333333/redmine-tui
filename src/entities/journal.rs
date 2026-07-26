use chrono::{DateTime, Local};

use crate::vos::{JournalDetail, JournalId};

#[derive(Clone)]
pub struct Journal {
    pub id: JournalId,
    pub user: String,
    pub updated_on: DateTime<Local>,
    pub details: Vec<JournalDetail>,
    pub notes: String,
}
