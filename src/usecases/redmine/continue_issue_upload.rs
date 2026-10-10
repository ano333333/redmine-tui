use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, IssueAction, Store};
use crate::usecases::UsecaseTask;
use crate::vos::issue_property_diff::same_issue_property;
use crate::vos::{IssueId, IssuePropertyDiff};

use super::upload_issue::preflight_issue_upload;

/// 競合popupで選んだ値で、Issueの保存を再開する。
///
/// 競合情報を破棄するActionを同期的にdispatchし、再試行用のdiffで保存前の取得からやり直すtaskを返す。
/// Storeのdiffは、保存前の取得で競合がなかった時点で再試行用のdiffに置き換わる。
///
/// # Panics
///
/// Issueに保存の競合情報がない場合にpanicする。
pub fn continue_issue_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: IssueId,
    selected_local_diffs: Vec<IssuePropertyDiff>,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let retry_diffs = retry_diffs(dispatcher.borrow().store(), id, &selected_local_diffs);
    dispatcher
        .borrow_mut()
        .dispatch(IssueAction::ClearUploadConflicts { id });
    Some(Box::pin(preflight_issue_upload(client, id, retry_diffs)))
}

/// popupの競合解決結果から、最新Issueで再試行するための一時的なdiffを作成する。
///
/// ローカル値を選択したpropertyはpopup表示時点のサーバー値を`before`にする。サーバー値を
/// 選択したpropertyは同じ値を`after`にし、再取得時に値が変化した場合だけ再び競合させる。
fn retry_diffs(
    store: &Store,
    id: IssueId,
    selected_local_diffs: &[IssuePropertyDiff],
) -> Vec<IssuePropertyDiff> {
    let (server_issue, conflicts) = store
        .try_get_issue_upload_conflict(id)
        .expect("Issueのアップロード続行には競合情報が必要です");

    let mut retry_diffs = store
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
            .map(|selected| server_issue.with_value_as_before(selected))
            .unwrap_or_else(|| server_issue.with_value_as_after(conflict))
    }));
    retry_diffs
}

#[cfg(test)]
mod tests {
    use crate::entities::IssueAggregate;
    use crate::stores::{Dispatcher, IssueAction};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::issue_property_diff::{IssueDescriptionDiff, IssueDueDateDiff};
    use crate::vos::{IssueId, IssuePropertyDiff};

    use super::retry_diffs;

    #[test]
    fn popupの選択結果からstoreを変更せず再試行用diffを作成する() {
        let id = IssueId::new(1);
        let mut dispatcher = uploading_dispatcher(id);
        let original_diffs = dispatcher.store().get_issue_property_diffs(id).to_vec();
        let local_description = original_diffs[0].clone();
        let conflicts = original_diffs[..2].to_vec();
        let mut server_issue = sample_issue_aggregate(1, "subject", 9.into(), None, None, None, 0);
        server_issue.issue.description = "server body".to_string();
        dispatcher.dispatch(IssueAction::UploadConflictsDetected {
            server_issue,
            conflicts,
            children: vec![],
        });
        dispatcher.consume_action();

        let retry_diffs = retry_diffs(dispatcher.store(), id, &[local_description.clone()]);

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
        let mut issue: IssueAggregate =
            sample_issue_aggregate(1, "subject", 1.into(), None, None, None, 0);
        issue.issue.description = "original body".to_string();
        crate::test_support::dispatch_loaded_issue(&mut dispatcher, issue);
        dispatcher.dispatch(IssueAction::UpdateDescription {
            id,
            body: "local body".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::UpdateStatus {
            id,
            status_id: 2.into(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::UpdateDueDate {
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
        dispatcher.dispatch(IssueAction::StartUpload { id });
        dispatcher.consume_action();
        dispatcher
    }
}
