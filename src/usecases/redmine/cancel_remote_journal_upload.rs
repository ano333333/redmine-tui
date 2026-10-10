use crate::stores::{Dispatcher, JournalAction};
use crate::vos::{IssueId, JournalId};

/// Remote Journalの保存の競合を破棄し、編集を残したまま保存を取りやめる。
pub fn cancel_remote_journal_upload(
    dispatcher: &mut Dispatcher,
    issue_id: IssueId,
    journal_id: JournalId,
) {
    dispatcher.dispatch(JournalAction::CancelRemoteUploadConflict {
        issue_id,
        journal_id,
    });
}
