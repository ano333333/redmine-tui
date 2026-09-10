use crate::stores::{Dispatcher, IssueAction};
use crate::vos::IssueId;

/// Issueの競合情報を破棄し、アップロードをキャンセルするActionを順番にdispatchする。
pub fn cancel_issue_upload(dispatcher: &mut Dispatcher, id: IssueId) {
    dispatcher.dispatch(IssueAction::ClearUploadConflicts { id });
    dispatcher.dispatch(IssueAction::CancelUpload { id });
}

#[cfg(test)]
mod tests {
    use crate::stores::{Dispatcher, IssueAction, IssueState};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::IssueId;

    use super::cancel_issue_upload;

    #[test]
    fn conflictを削除してからissue_uploadをキャンセルする() {
        let id = IssueId::new(1);
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(IssueAction::Load { id });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::UpdateDescription {
            id,
            body: "local body".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::StartUpload { id });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::UploadConflictsDetected {
            server_issue: sample_issue_aggregate(1, "server issue", 1.into(), None, None, None, 0),
            conflicts: dispatcher.store().get_issue_property_diffs(id).to_vec(),
        });
        dispatcher.consume_action();

        cancel_issue_upload(&mut dispatcher, id);

        assert_eq!(dispatcher.consume_actinos_len(), 2);
        dispatcher.consume_action();
        assert!(dispatcher.store().get_issue_upload_conflict(id).is_none());
        assert_eq!(
            dispatcher.store().get_issue(id).unwrap().1,
            &IssueState::Uploading
        );

        dispatcher.consume_action();
        assert_eq!(
            dispatcher.store().get_issue(id).unwrap().1,
            &IssueState::Edited
        );
        assert_eq!(dispatcher.store().get_issue_property_diffs(id).len(), 1);
    }
}
