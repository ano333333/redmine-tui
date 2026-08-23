use crate::stores::{Action, Dispatcher};
use crate::vos::{IssueId, IssuePropertyDiff};

use super::fetch_issue_with_conflicts::{
    same_issue_property, with_server_value_as_after, with_server_value_as_before,
};

/// popupの競合解決結果から、最新Issueで再試行するための一時的なdiffを作成する。
///
/// ローカル値を選択したpropertyはpopup表示時点のサーバー値を`before`にする。サーバー値を
/// 選択したpropertyは同じ値を`after`にし、再取得時に値が変化した場合だけ再び競合させる。
/// Storeが保持する元のdiffは変更せず、競合情報を破棄するActionだけをdispatchする。
pub fn continue_issue_upload(
    dispatcher: &mut Dispatcher,
    id: IssueId,
    selected_local_diffs: Vec<IssuePropertyDiff>,
) -> Vec<IssuePropertyDiff> {
    let (server_issue, conflicts) = dispatcher
        .store()
        .get_issue_upload_conflict(id)
        .map(|(issue, conflicts)| (issue.clone(), conflicts.to_vec()))
        .expect("Issueのアップロード続行には競合情報が必要です");

    let mut retry_diffs = dispatcher
        .store()
        .get_issue_property_diffs(id)
        .iter()
        .filter(|diff| {
            !conflicts
                .iter()
                .any(|conflict| same_issue_property(diff, conflict))
        })
        .cloned()
        .collect::<Vec<_>>();
    retry_diffs.extend(conflicts.iter().map(|conflict| {
        selected_local_diffs
            .iter()
            .find(|selected| same_issue_property(conflict, selected))
            .map(|selected| with_server_value_as_before(&server_issue, selected))
            .unwrap_or_else(|| with_server_value_as_after(&server_issue, conflict))
    }));

    dispatcher.dispatch(Action::ClearIssueUploadConflicts { id });
    retry_diffs
}

#[cfg(test)]
mod tests {
    use crate::entities::Issue;
    use crate::stores::{Action, Dispatcher};
    use crate::test_support::sample_issue;
    use crate::vos::issue_property_diff::{IssueDescriptionDiff, IssueDueDateDiff};
    use crate::vos::{IssueId, IssuePropertyDiff};

    use super::continue_issue_upload;

    #[test]
    fn popupの選択結果からstoreを変更せず再試行用diffを作成する() {
        let id = IssueId::new(1);
        let mut dispatcher = uploading_dispatcher(id);
        let original_diffs = dispatcher.store().get_issue_property_diffs(id).to_vec();
        let local_description = original_diffs[0].clone();
        let conflicts = original_diffs[..2].to_vec();
        let mut server_issue = dispatcher.store().get_issue(id).unwrap().0.clone();
        server_issue.description = "server body".to_string();
        server_issue.status_id = 9.into();
        dispatcher.dispatch(Action::IssueUploadConflictsDetected {
            server_issue,
            conflicts,
        });
        dispatcher.consume_action();

        let retry_diffs =
            continue_issue_upload(&mut dispatcher, id, vec![local_description.clone()]);

        assert_eq!(dispatcher.consume_actinos_len(), 1);
        assert_eq!(
            dispatcher.store().get_issue_property_diffs(id),
            original_diffs
        );
        assert_eq!(
            retry_diffs,
            vec![
                original_diffs[2].clone(),
                IssuePropertyDiff::Description(IssueDescriptionDiff {
                    before: "server body".to_string(),
                    after: match local_description {
                        IssuePropertyDiff::Description(diff) => diff.after,
                        _ => unreachable!(),
                    },
                }),
                IssuePropertyDiff::StatusId(crate::vos::issue_property_diff::IssueStatusIdDiff {
                    before: 1.into(),
                    after: 9.into(),
                },),
            ]
        );
    }

    fn uploading_dispatcher(id: IssueId) -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        let mut issue: Issue = sample_issue(1, "subject", 1.into(), None, None, None, 0);
        issue.description = "original body".to_string();
        dispatcher.dispatch(Action::SyncIssue { issue });
        dispatcher.consume_action();
        dispatcher.dispatch(Action::UpdateIssue {
            id,
            body: "local body".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(Action::UpdateIssueStatus {
            id,
            status_id: 2.into(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(Action::UpdateIssueDueDate {
            id,
            due_date: Some(crate::test_support::local_datetime(
                "2026-08-30T00:00:00+09:00",
            )),
        });
        dispatcher.consume_action();
        assert!(matches!(
            dispatcher.store().get_issue_property_diffs(id)[2],
            IssuePropertyDiff::DueDate(IssueDueDateDiff { .. })
        ));
        dispatcher.dispatch(Action::StartIssueUpload { id });
        dispatcher.consume_action();
        dispatcher
    }
}
