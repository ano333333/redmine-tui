use crate::usecases::IssueCreationId;
use crate::vos::{IssueId, JournalId};

/// Usecaseで一度だけ起きたことを表す。発したUsecaseは受け取り手を知らない。
#[derive(Clone, Debug, PartialEq)]
pub enum UsecaseEvent {
    IssueCreated {
        creation_id: IssueCreationId,
        id: IssueId,
    },
    Failure(UsecaseFailure),
}

/// Issueの作成で失敗した段階。
#[derive(Clone, Debug, PartialEq)]
pub enum IssueCreateFailure {
    AttachmentFileRead { filename: String, reason: String },
    AttachmentFileUpload { filename: String, reason: String },
    Create { reason: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum UsecaseFailure {
    IssueCreate {
        creation_id: IssueCreationId,
        cause: IssueCreateFailure,
    },
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
    DeletedJournalUpload {
        original_id: JournalId,
        reason: String,
    },
    /// PUTは成功し、確認の取得だけが失敗した。
    DeletedJournalUploadConfirm {
        original_id: JournalId,
        reason: String,
    },
    AttachmentFileRead {
        issue_id: IssueId,
        reason: String,
    },
    AttachmentFileUpload {
        issue_id: IssueId,
        reason: String,
    },
    AttachmentAttach {
        issue_id: IssueId,
        reason: String,
    },
    /// 添付は成功し、確認の取得だけが失敗した。
    AttachmentAttachConfirm {
        issue_id: IssueId,
        reason: String,
    },
}
