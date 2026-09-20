use chrono::{DateTime, Local};

use crate::vos::{IssueId, JournalDetail, JournalId};

/// Redmineから取得し、[`JournalId`]で識別するJournal。
#[derive(Clone)]
pub struct Journal {
    pub id: JournalId,
    pub issue_id: IssueId,
    pub user: String,
    pub updated_on: Option<DateTime<Local>>,
    pub details: Vec<JournalDetail>,
    pub notes: String,
}

/// Redmine由来のIDをまだ確認できていない、Issue単位のJournal。
///
/// Issueごとに最大1件だけ保持し、[`IssueId`]で識別する。Redmineへの作成が成功しても、
/// 作成後の再取得に失敗した場合は引き続きこの型で保持する。
#[derive(Clone)]
pub struct LocalJournal {
    pub issue_id: IssueId,
    pub notes: String,
}
