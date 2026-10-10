use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::base::FetchedIssue;
use crate::clients::redmine::{IssueUpdate, RedmineClient};
use crate::stores::{Action, Dispatcher, IssueAction, IssueState, NoticeAction, NoticeId};
use crate::usecases::{UsecaseOutput, UsecaseRequest, UsecaseTask};
use crate::vos::{EntityIdValue, IssueId, IssuePropertyDiff};

/// 編集済みのIssueのuploadを開始する。
///
/// 呼び出し時に状態を検証し、`StartUpload`を同期的にqueueへ追加してproperty diffをsnapshotする。
/// 返却したtaskは保存前の取得を行う。PUTと確認の取得は、それぞれ前の段階の完了Actionを
/// 適用した後に後続要求として起動する。
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

    Some(Box::pin(preflight_issue_upload(client, id, diffs)))
}

/// 保存前の取得で`diffs`がサーバーの値と競合しないかを確かめる。
///
/// 競合した場合は、取得したJournalと子一覧とともに競合を返し、PUTしない。競合しなければ、
/// 取得結果を基準値にする完了Actionと、`diffs`をPUTする後続要求を返す。`diffs`は
/// 競合の続行で作った再試行用のdiffの場合があり、Storeのdiffとは限らないため、要求に載せて運ぶ。
pub(super) async fn preflight_issue_upload<C>(
    client: Arc<C>,
    id: IssueId,
    diffs: Vec<IssuePropertyDiff>,
) -> UsecaseOutput
where
    C: RedmineClient,
{
    let preflight = match fetch_issue_for_upload(client.as_ref(), id).await {
        Ok(fetched) => fetched,
        Err(message) => return issue_upload_failure_actions(id, message).into(),
    };
    if let Err(conflicts) = preflight.aggregate.with_property_diffs(&diffs) {
        return vec![
            IssueAction::UploadConflictsDetected {
                server_issue: preflight.aggregate,
                conflicts,
                children: preflight.children,
            }
            .into(),
        ]
        .into();
    }
    UsecaseOutput {
        actions: vec![
            IssueAction::UploadPreflightSucceeded {
                server_issue: preflight.aggregate,
                children: preflight.children,
                diffs: diffs.clone(),
            }
            .into(),
        ],
        requests: vec![UsecaseRequest::PutIssueUpload { id, diffs }],
    }
}

/// 保存前の取得を終えたIssueの属性`diffs`をPUTする。
///
/// 成功したら、送信したdiffを基準値へ確定する完了Actionと、確認の取得の後続要求を返す。
///
/// # Panics
///
/// Issueが競合情報のないUploadingでない場合にpanicする。
pub fn put_issue_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: IssueId,
    diffs: Vec<IssuePropertyDiff>,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    assert_uploading_without_conflict(&dispatcher.borrow(), id);
    Some(Box::pin(async move {
        if let Err(error) = client
            .update_issue(id, &IssueUpdate::from_diffs(&diffs))
            .await
        {
            return issue_upload_failure_actions(id, error.to_string()).into();
        }
        UsecaseOutput {
            actions: vec![IssueAction::UploadPutSucceeded { id, diffs }.into()],
            requests: vec![UsecaseRequest::ConfirmIssueUpload { id }],
        }
    }))
}

/// PUTを終えたIssueを取得し直し、更新日時やRedmineが追加したJournalを取り込む。
///
/// 取得に失敗しても、PUTの成功は取り消さず、PUTで確定した基準値のまま保存を終える。
///
/// # Panics
///
/// Issueが競合情報のないUploadingでない場合にpanicする。
pub fn confirm_issue_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    id: IssueId,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    assert_uploading_without_conflict(&dispatcher.borrow(), id);
    Some(Box::pin(async move {
        let actions = match fetch_issue_for_upload(client.as_ref(), id).await {
            Ok(confirmed) => vec![
                IssueAction::UploadSucceeded {
                    issue: confirmed.aggregate,
                    children: confirmed.children,
                }
                .into(),
            ],
            Err(reason) => vec![
                NoticeAction::Push {
                    id: NoticeId::new(),
                    message: format!(
                        "Issue #{id}を保存しましたが、確認の取得に失敗しました: {reason}"
                    ),
                }
                .into(),
                IssueAction::UploadConfirmFailed { id }.into(),
            ],
        };
        actions.into()
    }))
}

fn assert_uploading_without_conflict(dispatcher: &Dispatcher, id: IssueId) {
    let store = dispatcher.store();
    assert!(
        store.try_get_issue_state(id) == Some(IssueState::Uploading)
            && store.try_get_issue_upload_conflict(id).is_none(),
        "issue {id} is not uploading without conflicts"
    );
}

/// 応答IDの不一致はサーバー側の外部データ異常として、取得失敗と同じく扱う。
async fn fetch_issue_for_upload(
    client: &impl RedmineClient,
    id: IssueId,
) -> Result<FetchedIssue, String> {
    match client.get_issue(id).await {
        Ok(fetched) if fetched.aggregate.issue.id == id => Ok(fetched),
        Ok(fetched) => Err(format!(
            "requested issue {} but Redmine returned issue {}",
            id.get(),
            fetched.aggregate.issue.id.get()
        )),
        Err(error) => Err(error.to_string()),
    }
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

    /// 説明を"server"から"local"へ編集して保存を始めた状態。
    fn uploading_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        crate::test_support::dispatch_loaded_issue(&mut dispatcher, issue("server"));
        for action in [
            IssueAction::UpdateDescription {
                id: ISSUE_ID,
                body: "local".to_string(),
            },
            IssueAction::StartUpload { id: ISSUE_ID },
        ] {
            dispatcher.dispatch(action);
            dispatcher.consume_action();
        }
        Rc::new(RefCell::new(dispatcher))
    }

    #[test]
    #[should_panic(expected = "uploading issue is not edited")]
    fn starting_an_upload_of_a_synced_issue_panics() {
        let mut dispatcher = Dispatcher::new();
        crate::test_support::dispatch_loaded_issue(&mut dispatcher, issue("server"));
        let client = Arc::new(StubClient::new(vec![], Ok(())));

        start_issue_upload(Rc::new(RefCell::new(dispatcher)), client, ISSUE_ID);
    }

    #[tokio::test]
    async fn preflight_without_conflicts_requests_a_put_of_the_diffs() {
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched("server", "before put"))],
            Ok(()),
        ));

        let output = preflight_issue_upload(client.clone(), ISSUE_ID, description_diff()).await;

        let [
            Action::Issue(IssueAction::UploadPreflightSucceeded {
                server_issue,
                children,
                diffs,
            }),
        ] = output.actions.as_slice()
        else {
            panic!("expected UploadPreflightSucceeded");
        };
        assert_eq!(server_issue.issue.description, "server");
        assert_eq!(subjects(children), vec!["before put"]);
        assert_eq!(diffs, &description_diff());
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::PutIssueUpload {
                id: ISSUE_ID,
                diffs: description_diff(),
            }]
        );
        assert!(client.updates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_preflight_fetch_of_another_issue_fails_without_putting() {
        let mut other = fetched("server", "before put");
        other.aggregate.issue.id = IssueId::new(99);
        let client = Arc::new(StubClient::new(vec![Ok(other)], Ok(())));

        let output = preflight_issue_upload(client.clone(), ISSUE_ID, description_diff()).await;

        let [
            Action::Notice(NoticeAction::Push { .. }),
            Action::Issue(IssueAction::FailUpload { id, message }),
        ] = output.actions.as_slice()
        else {
            panic!("expected a notice and FailUpload");
        };
        assert_eq!(*id, ISSUE_ID);
        assert_eq!(message, "requested issue 1 but Redmine returned issue 99");
        assert!(output.requests.is_empty());
    }

    #[tokio::test]
    async fn a_property_conflict_returns_the_fetched_issue_and_children_without_putting() {
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched("changed on server", "fetched"))],
            Ok(()),
        ));

        let output = preflight_issue_upload(client, ISSUE_ID, description_diff()).await;

        let [
            Action::Issue(IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
                children,
            }),
        ] = output.actions.as_slice()
        else {
            panic!("expected UploadConflictsDetected");
        };
        assert_eq!(server_issue.issue.description, "changed on server");
        assert_eq!(conflicts, &description_diff());
        assert_eq!(subjects(children), vec!["fetched"]);
        assert!(output.requests.is_empty());
    }

    #[tokio::test]
    async fn a_put_puts_only_the_edits_and_requests_the_confirmation() {
        let client = Arc::new(StubClient::new(vec![], Ok(())));

        let output = complete(put_issue_upload(
            uploading_dispatcher(),
            client.clone(),
            ISSUE_ID,
            description_diff(),
        ))
        .await;

        assert!(matches!(
            output.actions.as_slice(),
            [Action::Issue(IssueAction::UploadPutSucceeded { id, diffs })]
                if *id == ISSUE_ID && *diffs == description_diff()
        ));
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::ConfirmIssueUpload { id: ISSUE_ID }]
        );
        assert_eq!(
            *client.updates.lock().unwrap(),
            vec![IssueUpdate {
                description: Some("local".to_string()),
                ..IssueUpdate::default()
            }]
        );
    }

    #[tokio::test]
    async fn a_put_failure_keeps_the_edits_without_confirming() {
        let client = Arc::new(StubClient::new(vec![], Err(offline())));

        let output = complete(put_issue_upload(
            uploading_dispatcher(),
            client,
            ISSUE_ID,
            description_diff(),
        ))
        .await;

        assert!(matches!(
            output.actions.as_slice(),
            [
                Action::Notice(NoticeAction::Push { .. }),
                Action::Issue(IssueAction::FailUpload { .. })
            ]
        ));
        assert!(output.requests.is_empty());
    }

    #[tokio::test]
    async fn a_confirmation_completes_with_the_confirmed_issue_and_children() {
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched("confirmed", "after put"))],
            Ok(()),
        ));

        let output = complete(confirm_issue_upload(
            uploading_dispatcher(),
            client.clone(),
            ISSUE_ID,
        ))
        .await;

        let [Action::Issue(IssueAction::UploadSucceeded { issue, children })] =
            output.actions.as_slice()
        else {
            panic!("expected UploadSucceeded");
        };
        assert_eq!(issue.issue.description, "confirmed");
        assert_eq!(subjects(children), vec!["after put"]);
        assert!(client.updates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_confirmation_failure_finishes_the_upload_without_putting_again() {
        let client = Arc::new(StubClient::new(vec![Err(offline())], Ok(())));

        let output = complete(confirm_issue_upload(
            uploading_dispatcher(),
            client.clone(),
            ISSUE_ID,
        ))
        .await;

        let [
            Action::Notice(NoticeAction::Push { message, .. }),
            Action::Issue(IssueAction::UploadConfirmFailed { id }),
        ] = output.actions.as_slice()
        else {
            panic!("expected a notice and UploadConfirmFailed");
        };
        assert_eq!(
            message,
            "Issue #1を保存しましたが、確認の取得に失敗しました: network error: offline"
        );
        assert_eq!(*id, ISSUE_ID);
        assert!(output.requests.is_empty());
        assert!(client.updates.lock().unwrap().is_empty());
    }

    async fn complete(task: Option<UsecaseTask>) -> UsecaseOutput {
        task.expect("usecase should start").await
    }
}
