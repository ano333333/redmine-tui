use chrono::{DateTime, Local};

#[derive(Clone)]
pub enum JournalDetailAttr {
    StatusId {
        old: String,
        new: String,
    },
    DueDate {
        old: DateTime<Local>,
        new: DateTime<Local>,
    },
    AssignedTo {
        old: Option<String>,
        new: Option<String>,
    }, // FIXME:
       // - tracker_id
       // - project_id
       // - subject
       // - description
       // - category_id
       // - assigned_to_id
       // - priority_id
       // - fixed_version_id
       // - author_id
       // - start_date
       // - done_ratio
       // - estimated_hours
       // - parent_id
       // - is_private
       // 用のstructの追加
}

#[derive(Clone)]
pub enum JournalDetail {
    Attr(JournalDetailAttr),
    // FIXME:
    // カスタムフィールド(cf)、添付ファイル(attachment)、リレーション(relation)用のstructの追加
}
