use crate::vos::{IssueId, JournalId};

/// Usecaseで一度だけ起きたことを表す。発したUsecaseは受け取り手を知らない。
#[derive(Clone, Debug, PartialEq)]
pub enum UsecaseEvent {
    Failure(UsecaseFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UsecaseFailure {
    IssueUpload {
        id: IssueId,
        reason: String,
    },
    /// PUTは成功し、確認の取得だけが失敗した。
    IssueUploadConfirm {
        id: IssueId,
        reason: String,
    },
    RemoteJournalUpload {
        journal_id: JournalId,
        reason: String,
    },
    /// PUTは成功し、確認の取得だけが失敗した。
    RemoteJournalUploadConfirm {
        journal_id: JournalId,
        reason: String,
    },
    LocalJournalUpload {
        issue_id: IssueId,
        reason: String,
    },
    /// PUTは成功し、確認の取得だけが失敗した。
    LocalJournalUploadConfirm {
        issue_id: IssueId,
        reason: String,
    },
}
