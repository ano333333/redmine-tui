use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::{IssueUpdate, RedmineClient};
use crate::stores::{Action, Dispatcher, IssueAction, IssueState, NoticeAction, NoticeId};
use crate::vos::{IssueId, IssuePropertyDiff};

/// 編集済みのIssueのuploadを開始する。
///
/// 呼び出し時に状態を検証し、`StartUpload`を同期的にqueueへ追加してproperty diffをsnapshotする。
/// 返却したFutureは競合確認GETとIssue PUTを行い、完了Actionを返す。
///
/// # Panics
///
/// Issueが未登録、またはEdited以外の場合にpanicする。
///
/// FIXME: trackerを変更するとステータスが自動でデフォルトに戻る？Issueの読み直しが必要な可能性
pub fn start_issue_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: IssueId,
) -> impl Future<Output = Vec<Action>> + 'static
where
    C: RedmineClient + Send + Sync + 'static,
{
    {
        let dispatcher = dispatcher.borrow();
        let (_, state) = dispatcher.store().get_issue(id);
        if state != IssueState::Edited {
            panic!("uploading issue is not edited");
        }
    }
    dispatcher
        .borrow_mut()
        .dispatch(IssueAction::StartUpload { id });
    let diffs = dispatcher
        .borrow()
        .store()
        .get_issue_property_diffs(id)
        .to_vec();

    async move { upload_issue_action(client.as_ref(), id, &diffs).await }
}

pub async fn upload_issue_action(
    client: &impl RedmineClient,
    id: IssueId,
    diffs: &[IssuePropertyDiff],
) -> Vec<Action> {
    let server_issue = match client.get_issue(id).await {
        Ok(fetched) => fetched.aggregate,
        Err(error) => {
            return issue_upload_failure_actions(id, error.to_string());
        }
    };
    let issue = match server_issue.with_property_diffs(diffs) {
        Ok(issue) => issue,
        Err(conflicts) => {
            return vec![
                IssueAction::UploadConflictsDetected {
                    server_issue,
                    conflicts,
                }
                .into(),
            ];
        }
    };
    if let Err(error) = client
        .update_issue(id, &IssueUpdate::from_diffs(diffs))
        .await
    {
        return issue_upload_failure_actions(id, error.to_string());
    }

    vec![IssueAction::Sync { issue }.into()]
}

// FIXME: usecaseがUI表示物(notice/toast)の文言を組み立てているのは設計上の負債である。
// 将来的にはIssueStoreの状態を見て判断するtoast component等を導入し、
// この処理をそちらへ移すべき。
fn issue_upload_failure_actions(id: IssueId, message: String) -> Vec<Action> {
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message: format!("Issue #{id}の保存に失敗しました: {message}"),
        }
        .into(),
        IssueAction::FailUpload { id, message }.into(),
    ]
}
