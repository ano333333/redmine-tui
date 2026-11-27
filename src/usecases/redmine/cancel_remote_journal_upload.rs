use crate::stores::{Dispatcher, JournalAction};
use crate::vos::{IssueId, JournalId};

/// キャンセル後もRemote Journalの編集内容は残る。
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
