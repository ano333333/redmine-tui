use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::{IssueUpdate, RedmineClient};
use crate::stores::{Action, Dispatcher, IssueAction, IssueState, NoticeAction, NoticeId};
use crate::usecases::UsecaseTask;
use crate::vos::{EntityIdValue, IssueId, IssuePropertyDiff};

/// 編集済みのIssueのuploadを開始する。
///
/// 呼び出し時に状態を検証し、`StartUpload`を同期的にqueueへ追加してproperty diffをsnapshotする。
/// 返却したFutureは保存前の取得、Issue PUT、確認の取得を行い、完了Actionを返す。
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
) -> Option<UsecaseTask>
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

    Some(Box::pin(async move {
        upload_issue_action(client.as_ref(), id, &diffs)
            .await
            .into()
    }))
}

/// 保存前の取得、Issue属性のPUT、確認の取得を順に行い、結果のActionを返す。
///
/// 保存前の取得でIssue属性が競合した場合はPUTせず、取得したJournalと子一覧とともに競合を返す。
/// PUT成功後に確認の取得だけが失敗した場合は、PUTを繰り返さないよう、保存前の取得値に
/// 送信した差分を適用した値を新しい基準値として完了させる。
// FIXME: 保存前の取得・PUT・確認の取得を1つのFutureで続けて実行し、結果を最後に1つのActionで
// 反映している。PUT成功をStoreへ確定してから確認の取得を始められず、確認の取得だけを
// 再試行する経路もない。リクエストごとに完了Actionを適用して次へ進む実行方式で分割する。
pub async fn upload_issue_action(
    client: &impl RedmineClient,
    id: IssueId,
    diffs: &[IssuePropertyDiff],
) -> Vec<Action> {
    let preflight = match client.get_issue(id).await {
        Ok(fetched) if fetched.aggregate.issue.id == id => fetched,
        Ok(fetched) => {
            return issue_upload_failure_actions(
                id,
                format!(
                    "requested issue {} but Redmine returned issue {}",
                    id.get(),
                    fetched.aggregate.issue.id.get()
                ),
            );
        }
        Err(error) => {
            return issue_upload_failure_actions(id, error.to_string());
        }
    };
    let applied = match preflight.aggregate.with_property_diffs(diffs) {
        Ok(applied) => applied,
        Err(conflicts) => {
            return vec![
                IssueAction::UploadConflictsDetected {
                    server_issue: preflight.aggregate,
                    conflicts,
                    children: preflight.children,
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

    let reason = match client.get_issue(id).await {
        Ok(confirmed) if confirmed.aggregate.issue.id == id => {
            return vec![
                IssueAction::UploadSucceeded {
                    issue: confirmed.aggregate,
                    children: confirmed.children,
                }
                .into(),
            ];
        }
        Ok(confirmed) => format!(
            "requested issue {} but Redmine returned issue {}",
            id.get(),
            confirmed.aggregate.issue.id.get()
        ),
        Err(error) => error.to_string(),
    };
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message: format!("Issue #{id}を保存しましたが、確認の取得に失敗しました: {reason}"),
        }
        .into(),
        IssueAction::UploadSucceeded {
            issue: applied,
            children: preflight.children,
        }
        .into(),
    ]
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use crate::clients::redmine::RedmineClientError;
    use crate::clients::redmine::base::FetchedIssue;
    use crate::entities::{
        Category, IssueAggregate, IssueChild, IssueStatus, Priority, Project, ProjectIssuesPage,
        TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crate::vos::{IssueStatusId, JournalId, ProjectId, TrackerId};

    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);

    fn issue(description: &str) -> IssueAggregate {
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.issue.description = description.to_string();
        issue
    }

    fn fetched(description: &str, child_subject: &str) -> FetchedIssue {
        FetchedIssue {
            aggregate: issue(description),
            children: vec![IssueChild {
                id: IssueId::new(2),
                tracker_id: TrackerId::new(1),
                subject: child_subject.to_string(),
                children: vec![],
            }],
        }
    }

    fn description_diff() -> Vec<IssuePropertyDiff> {
        vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "server".to_string(),
            after: "local".to_string(),
        })]
    }

    fn offline() -> RedmineClientError {
        RedmineClientError::Network {
            reason: "offline".to_string(),
        }
    }

    /// GETの結果を呼び出し順に返し、PUTした更新要求を記録する。
    struct StubClient {
        get_results: Mutex<VecDeque<Result<FetchedIssue, RedmineClientError>>>,
        put_result: Result<(), RedmineClientError>,
        updates: Mutex<Vec<IssueUpdate>>,
    }

    impl StubClient {
        fn new(
            get_results: Vec<Result<FetchedIssue, RedmineClientError>>,
            put_result: Result<(), RedmineClientError>,
        ) -> Self {
            Self {
                get_results: Mutex::new(get_results.into()),
                put_result,
                updates: Mutex::new(Vec::new()),
            }
        }
    }

    impl RedmineClient for StubClient {
        async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
            self.get_results
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected GET")
        }

        async fn update_issue(
            &self,
            _: IssueId,
            update: &IssueUpdate,
        ) -> Result<(), RedmineClientError> {
            self.updates.lock().unwrap().push(update.clone());
            self.put_result.clone()
        }

        async fn update_issue_notes(&self, _: IssueId, _: &str) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_journal_notes(
            &self,
            _: JournalId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
            unreachable!()
        }

        async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError> {
            unreachable!()
        }

        async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError> {
            unreachable!()
        }

        async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError> {
            unreachable!()
        }

        async fn get_project_issues(
            &self,
            _: ProjectId,
            _: std::num::NonZeroUsize,
        ) -> Result<ProjectIssuesPage, RedmineClientError> {
            unreachable!()
        }

        async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError> {
            unreachable!()
        }

        async fn get_time_entity_activities(
            &self,
        ) -> Result<Vec<TimeEntityActivity>, RedmineClientError> {
            unreachable!()
        }

        async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError> {
            unreachable!()
        }

        async fn get_users(&self) -> Result<Vec<User>, RedmineClientError> {
            unreachable!()
        }
    }

    fn subjects(children: &[IssueChild]) -> Vec<&str> {
        children
            .iter()
            .map(|child| child.subject.as_str())
            .collect()
    }

    #[tokio::test]
    async fn completes_with_the_confirmed_issue_and_children_after_putting_only_the_edits() {
        let client = StubClient::new(
            vec![
                Ok(fetched("server", "before put")),
                Ok(fetched("confirmed", "after put")),
            ],
            Ok(()),
        );

        let actions = upload_issue_action(&client, ISSUE_ID, &description_diff()).await;

        let [Action::Issue(IssueAction::UploadSucceeded { issue, children })] = actions.as_slice()
        else {
            panic!("expected UploadSucceeded");
        };
        assert_eq!(issue.issue.description, "confirmed");
        assert_eq!(subjects(children), vec!["after put"]);
        assert_eq!(
            *client.updates.lock().unwrap(),
            vec![IssueUpdate {
                description: Some("local".to_string()),
                ..IssueUpdate::default()
            }]
        );
    }

    #[tokio::test]
    async fn a_preflight_fetch_of_another_issue_fails_without_putting() {
        let mut other = fetched("server", "before put");
        other.aggregate.issue.id = IssueId::new(99);
        let client = StubClient::new(vec![Ok(other)], Ok(()));

        let actions = upload_issue_action(&client, ISSUE_ID, &description_diff()).await;

        let [
            Action::Notice(NoticeAction::Push { .. }),
            Action::Issue(IssueAction::FailUpload { id, message }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and FailUpload");
        };
        assert_eq!(*id, ISSUE_ID);
        assert_eq!(message, "requested issue 1 but Redmine returned issue 99");
        assert!(client.updates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_confirmation_failure_completes_with_the_sent_edits_without_putting_again() {
        let client = StubClient::new(
            vec![Ok(fetched("server", "before put")), Err(offline())],
            Ok(()),
        );

        let actions = upload_issue_action(&client, ISSUE_ID, &description_diff()).await;

        let [
            Action::Notice(NoticeAction::Push { message, .. }),
            Action::Issue(IssueAction::UploadSucceeded { issue, children }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and UploadSucceeded");
        };
        assert_eq!(
            message,
            "Issue #1を保存しましたが、確認の取得に失敗しました: network error: offline"
        );
        assert_eq!(issue.issue.description, "local");
        assert_eq!(subjects(children), vec!["before put"]);
        assert_eq!(client.updates.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_property_conflict_returns_the_fetched_issue_and_children_without_putting() {
        let client = StubClient::new(vec![Ok(fetched("changed on server", "fetched"))], Ok(()));

        let actions = upload_issue_action(&client, ISSUE_ID, &description_diff()).await;

        let [
            Action::Issue(IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
                children,
            }),
        ] = actions.as_slice()
        else {
            panic!("expected UploadConflictsDetected");
        };
        assert_eq!(server_issue.issue.description, "changed on server");
        assert_eq!(conflicts, &description_diff());
        assert_eq!(subjects(children), vec!["fetched"]);
        assert!(client.updates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_put_failure_keeps_the_edits_without_confirming() {
        let client = StubClient::new(vec![Ok(fetched("server", "before put"))], Err(offline()));

        let actions = upload_issue_action(&client, ISSUE_ID, &description_diff()).await;

        assert!(matches!(
            actions.as_slice(),
            [
                Action::Notice(NoticeAction::Push { .. }),
                Action::Issue(IssueAction::FailUpload { .. })
            ]
        ));
        assert!(client.get_results.lock().unwrap().is_empty());
    }
}
