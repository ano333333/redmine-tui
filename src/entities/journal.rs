use chrono::{DateTime, Local};

pub enum Journal {
    Property {
        id: u16,
        creator: String,
        target: String,
        old: String,
        new: String,
        updated_at: DateTime<Local>,
    },
    Comment {
        id: u16,
        creator: String,
        updated_at: DateTime<Local>,
        body: String,
    },
}
