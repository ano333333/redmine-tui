use crate::vos::{IssueId, LocalJournalId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalJournal {
    pub id: LocalJournalId,
    pub issue_id: IssueId,
    pub notes: String,
}
