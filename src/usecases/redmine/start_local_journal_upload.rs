use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, Dispatcher, IssueState, JournalAction, LocalJournalState, NoticeAction, NoticeId,
};
use crate::vos::{EntityIdValue, IssueId};

/// Local JournalのPUT結果に応じたActionを生成するFuture。
pub type StartLocalJournalUploadFuture =
    Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

fn local_journal_upload_failure_actions(
    issue_id: IssueId,
    notice_message: String,
    message: String,
) -> Vec<Action> {
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message: notice_message,
            created_at: chrono::Local::now(),
        }
        .into(),
        JournalAction::FailLocalUpload { issue_id, message }.into(),
    ]
}

fn local_journal_put_failure_actions(issue_id: IssueId, message: String) -> Vec<Action> {
    local_journal_upload_failure_actions(
        issue_id,
        format!("Local Journalの保存に失敗しました: {message}"),
        message,
    )
}

/// PUT成功後の確認GETが失敗したときのActionを生成する。
///
/// この時点ではサーバー側にnotesが追加済みの可能性があるため、PUT失敗時とは異なる文言にして
/// 再試行が重複投稿になりうることを利用者へ伝える。
fn local_journal_fetch_failure_actions(issue_id: IssueId, reason: String) -> Vec<Action> {
    let message = format!(
        "Local Journalの保存は完了した可能性がありますが、確認の取得に失敗しました: {reason}"
    );
    local_journal_upload_failure_actions(issue_id, message.clone(), message)
}

/// 未保存のLocal Journalのuploadを開始する。
///
/// `StartLocalUpload`はFutureをpollする前に同期的にqueueへ追加する。返却したFutureは
/// notes追加のPUTと、その結果を確認するためのIssue GETを順に行う。
///
/// # Panics
///
/// Issueがupload中、対象Local Journalが未登録またはLocalOnly以外、もしくは同じIssueの
/// Remote Journalがupload中の場合にpanicする。
pub fn start_local_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
) -> StartLocalJournalUploadFuture
where
    C: RedmineClient + Send + Sync + 'static,
{
    let notes = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        if matches!(
            store.try_get_issue_state(issue_id),
            Some(IssueState::Uploading)
        ) {
            panic!("cannot start local journal upload while issue {issue_id} is uploading");
        }
        let entry = store
            .get_local_journal(issue_id)
            .unwrap_or_else(|| panic!("local journal is not registered for issue {issue_id}"));
        if !matches!(entry.state, LocalJournalState::LocalOnly { .. }) {
            panic!("cannot start local journal upload unless it is local only");
        }
        // 非同期処理を作る前にも検査し、StoreのAction入口での排他検査と合わせて
        // usecaseの直接呼び出しと直接dispatchの両方を拒否する。
        if store.has_uploading_journal(issue_id) {
            panic!(
                "cannot start local journal upload while another journal of issue {issue_id} is uploading"
            );
        }
        entry.journal.notes.clone()
    };

    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartLocalUpload { issue_id });

    Box::pin(async move { upload_local_journal(client.as_ref(), issue_id, notes).await })
}

async fn upload_local_journal<C>(client: &C, issue_id: IssueId, notes: String) -> Vec<Action>
where
    C: RedmineClient + Send + Sync + 'static,
{
    if let Err(error) = client.update_issue_notes(issue_id, &notes).await {
        return local_journal_put_failure_actions(issue_id, error.to_string());
    }
    let fetched = match client.get_issue(issue_id).await {
        Ok(fetched) => fetched,
        Err(error) => {
            return local_journal_fetch_failure_actions(issue_id, error.to_string());
        }
    };
    // 別Issueのレスポンスを確認結果として扱うと、他Issueのjournalを根拠に完了させてしまうため、
    // 取得失敗と同じ扱いにする。
    if fetched.aggregate.issue.id != issue_id {
        return local_journal_fetch_failure_actions(
            issue_id,
            format!(
                "requested issue {} but Redmine returned issue {}",
                issue_id.get(),
                fetched.aggregate.issue.id.get()
            ),
        );
    }
    vec![
        JournalAction::CompleteLocalUploadWithFetched {
            issue_id,
            journals: fetched.journals,
        }
        .into(),
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::clients::redmine::RedmineClientError;
    use crate::clients::redmine::base::FetchedIssue;
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Journal, Priority, Project, ProjectIssuesPage,
        TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{IssueAction, JournalUploadFailure};
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::{IssueStatusId, JournalId, ProjectId};

    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);

    fn remote_journal(id: u16, notes: &str) -> Journal {
        Journal {
            id: JournalId::new(id),
            issue_id: ISSUE_ID,
            user: "alice".to_string(),
            updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
            details: vec![],
            notes: notes.to_string(),
        }
    }

    struct StubClient {
        result: Result<(), RedmineClientError>,
        get_result: Result<u16, RedmineClientError>,
        journals: Vec<Journal>,
        request: Mutex<Option<(IssueId, String)>>,
        get_requests: Mutex<Vec<IssueId>>,
    }

    impl StubClient {
        fn succeeds() -> Self {
            Self {
                result: Ok(()),
                get_result: Ok(1),
                journals: vec![],
                request: Mutex::new(None),
                get_requests: Mutex::new(Vec::new()),
            }
        }

        fn succeeds_with_journals(journals: Vec<Journal>) -> Self {
            Self {
                journals,
                ..Self::succeeds()
            }
        }

        fn succeeds_with_fetched_issue_id(issue_id: u16, journals: Vec<Journal>) -> Self {
            Self {
                get_result: Ok(issue_id),
                journals,
                ..Self::succeeds()
            }
        }

        fn fails() -> Self {
            Self {
                result: Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
                ..Self::succeeds()
            }
        }

        fn fails_to_get() -> Self {
            Self {
                get_result: Err(RedmineClientError::Network {
                    reason: "offline".to_string(),
                }),
                ..Self::succeeds()
            }
        }
    }

    impl RedmineClient for StubClient {
        async fn update_issue_notes(
            &self,
            issue_id: IssueId,
            notes: &str,
        ) -> Result<(), RedmineClientError> {
            *self.request.lock().unwrap() = Some((issue_id, notes.to_string()));
            self.result.clone()
        }

        async fn get_issue(&self, issue_id: IssueId) -> Result<FetchedIssue, RedmineClientError> {
            self.get_requests.lock().unwrap().push(issue_id);
            let fetched_issue_id = self.get_result.clone()?;
            Ok(FetchedIssue {
                aggregate: sample_issue_aggregate(
                    fetched_issue_id,
                    "subject",
                    IssueStatusId::new(1),
                    None,
                    None,
                    None,
                    0,
                ),
                journals: self.journals.clone(),
            })
        }

        async fn update_issue(&self, _: &IssueAggregate) -> Result<(), RedmineClientError> {
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

    fn local_only_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(JournalAction::CreateLocal { issue_id: ISSUE_ID });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID,
            notes: "local notes".to_string(),
        });
        dispatcher.consume_action();
        Rc::new(RefCell::new(dispatcher))
    }

    fn assert_start_panics(dispatcher: Rc<RefCell<Dispatcher>>) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            start_local_journal_upload(dispatcher, Arc::new(StubClient::succeeds()), ISSUE_ID)
        }));
        assert!(result.is_err());
    }

    #[test]
    fn start_dispatches_before_the_future_is_polled() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds());

        let future = start_local_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID);

        assert!(client.request.lock().unwrap().is_none());
        dispatcher.borrow_mut().consume_action();
        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_local_journal(ISSUE_ID)
                .unwrap()
                .state,
            LocalJournalState::Uploading
        ));
        drop(future);
    }

    #[test]
    fn start_panics_when_the_local_journal_is_uploading() {
        let dispatcher = local_only_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartLocalUpload { issue_id: ISSUE_ID });
        dispatcher.borrow_mut().consume_action();

        assert_start_panics(dispatcher);
    }

    #[test]
    fn start_panics_when_the_issue_is_uploading() {
        let dispatcher = local_only_dispatcher();
        let issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::Sync { issue });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateDescription {
                id: ISSUE_ID,
                body: "edited".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartUpload { id: ISSUE_ID });
        dispatcher.borrow_mut().consume_action();

        assert_start_panics(dispatcher);
    }

    #[test]
    fn start_panics_when_a_remote_journal_is_uploading() {
        let dispatcher = local_only_dispatcher();
        let journal_id = JournalId::new(10);
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::SyncFetched {
                issue_id: ISSUE_ID,
                journals: vec![Journal {
                    id: journal_id,
                    issue_id: ISSUE_ID,
                    user: "alice".to_string(),
                    updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
                    details: vec![],
                    notes: "remote".to_string(),
                }],
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::EditRemoteNotes {
                issue_id: ISSUE_ID,
                journal_id,
                notes: "edited".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::StartRemoteUpload {
                issue_id: ISSUE_ID,
                journal_id,
            });
        dispatcher.borrow_mut().consume_action();

        assert_start_panics(dispatcher);
    }

    #[tokio::test]
    async fn put_success_sends_the_local_notes() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds());

        let actions = start_local_journal_upload(dispatcher, client.clone(), ISSUE_ID).await;

        assert_eq!(actions.len(), 1);
        assert_eq!(
            *client.request.lock().unwrap(),
            Some((ISSUE_ID, "local notes".to_string()))
        );
    }

    #[tokio::test]
    async fn put_success_fetches_the_issue_journals_and_returns_the_completion_action() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds_with_journals(vec![
            remote_journal(10, "remote"),
            remote_journal(11, "local notes"),
        ]));

        let actions = start_local_journal_upload(dispatcher, client.clone(), ISSUE_ID).await;

        assert_eq!(*client.get_requests.lock().unwrap(), vec![ISSUE_ID]);
        assert_eq!(actions.len(), 1);
        let Action::Journal(JournalAction::CompleteLocalUploadWithFetched { issue_id, journals }) =
            &actions[0]
        else {
            panic!("expected complete local upload action");
        };
        assert_eq!(*issue_id, ISSUE_ID);
        assert_eq!(
            journals
                .iter()
                .map(|journal| journal.id)
                .collect::<Vec<_>>(),
            vec![JournalId::new(10), JournalId::new(11)]
        );
    }

    #[tokio::test]
    async fn the_completion_action_replaces_the_local_journal_with_the_fetched_remote_journals() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds_with_journals(vec![
            remote_journal(10, "remote"),
            remote_journal(11, "local notes"),
        ]));

        let actions =
            start_local_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID).await;
        dispatcher.borrow_mut().consume_action();
        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
        }
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        assert!(store.get_local_journal(ISSUE_ID).is_none());
        assert_eq!(
            store
                .get_remote_journals(ISSUE_ID)
                .iter()
                .map(|entry| entry.journal.id)
                .collect::<Vec<_>>(),
            vec![JournalId::new(10), JournalId::new(11)]
        );
    }

    #[tokio::test]
    async fn put_failure_does_not_fetch_the_issue() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::fails());

        start_local_journal_upload(dispatcher, client.clone(), ISSUE_ID).await;

        assert!(client.get_requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_failure_returns_failure_actions_whose_message_says_the_put_may_have_succeeded() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::fails_to_get());

        let actions =
            start_local_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID).await;
        dispatcher.borrow_mut().consume_action();

        assert_eq!(*client.get_requests.lock().unwrap(), vec![ISSUE_ID]);
        assert_eq!(actions.len(), 2);
        let Action::Notice(NoticeAction::Push { message, .. }) = &actions[0] else {
            panic!("expected notice action");
        };
        assert_eq!(
            message,
            "Local Journalの保存は完了した可能性がありますが、確認の取得に失敗しました: network error: offline"
        );
        let Action::Journal(JournalAction::FailLocalUpload { issue_id, message }) = &actions[1]
        else {
            panic!("expected fail local upload action");
        };
        assert_eq!(*issue_id, ISSUE_ID);
        assert_eq!(
            message,
            "Local Journalの保存は完了した可能性がありますが、確認の取得に失敗しました: network error: offline"
        );

        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
        }
        dispatcher.borrow_mut().consume_action();
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_local_journal(ISSUE_ID).unwrap();
        assert_eq!(entry.journal.notes, "local notes");
        let LocalJournalState::LocalOnly {
            failure: Some(failure),
        } = &entry.state
        else {
            panic!("expected local only state with a failure");
        };
        assert_eq!(
            failure.message,
            "Local Journalの保存は完了した可能性がありますが、確認の取得に失敗しました: network error: offline"
        );
    }

    #[tokio::test]
    async fn get_failure_keeps_the_existing_remote_journals() {
        let dispatcher = local_only_dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::SyncFetched {
                issue_id: ISSUE_ID,
                journals: vec![remote_journal(10, "remote")],
            });
        dispatcher.borrow_mut().consume_action();
        let client = Arc::new(StubClient {
            journals: vec![remote_journal(10, "remote"), remote_journal(11, "created")],
            ..StubClient::fails_to_get()
        });

        let actions = start_local_journal_upload(dispatcher.clone(), client, ISSUE_ID).await;
        dispatcher.borrow_mut().consume_action();
        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
        }
        dispatcher.borrow_mut().consume_action();
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let remotes = dispatcher.store().get_remote_journals(ISSUE_ID);
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].journal.id, JournalId::new(10));
        assert_eq!(remotes[0].journal.notes, "remote");
    }

    #[tokio::test]
    async fn a_fetched_issue_id_mismatch_returns_failure_actions_with_both_ids() {
        let dispatcher = local_only_dispatcher();
        let client = Arc::new(StubClient::succeeds_with_fetched_issue_id(
            99,
            vec![remote_journal(11, "local notes")],
        ));

        let actions = start_local_journal_upload(dispatcher.clone(), client, ISSUE_ID).await;
        dispatcher.borrow_mut().consume_action();

        assert_eq!(actions.len(), 2);
        let Action::Journal(JournalAction::FailLocalUpload { issue_id, message }) = &actions[1]
        else {
            panic!("expected fail local upload action");
        };
        assert_eq!(*issue_id, ISSUE_ID);
        assert_eq!(
            message,
            "Local Journalの保存は完了した可能性がありますが、確認の取得に失敗しました: requested issue 1 but Redmine returned issue 99"
        );

        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
        }
        dispatcher.borrow_mut().consume_action();
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_local_journal(ISSUE_ID).unwrap();
        assert_eq!(entry.journal.notes, "local notes");
        assert!(matches!(
            entry.state,
            LocalJournalState::LocalOnly { failure: Some(_) }
        ));
        assert!(dispatcher.store().get_remote_journals(ISSUE_ID).is_empty());
    }

    #[tokio::test]
    async fn put_failure_returns_failure_actions_that_restore_notes() {
        let dispatcher = local_only_dispatcher();
        let actions =
            start_local_journal_upload(dispatcher.clone(), Arc::new(StubClient::fails()), ISSUE_ID)
                .await;
        dispatcher.borrow_mut().consume_action();

        assert_eq!(actions.len(), 2);
        let Action::Notice(NoticeAction::Push { message, .. }) = &actions[0] else {
            panic!("expected notice action");
        };
        assert_eq!(
            message,
            "Local Journalの保存に失敗しました: network error: offline"
        );
        let Action::Journal(JournalAction::FailLocalUpload { issue_id, message }) = &actions[1]
        else {
            panic!("expected fail local upload action");
        };
        assert_eq!(*issue_id, ISSUE_ID);
        assert_eq!(message, "network error: offline");
        dispatcher
            .borrow_mut()
            .dispatch(actions.into_iter().nth(1).unwrap());
        dispatcher.borrow_mut().consume_action();
        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_local_journal(ISSUE_ID).unwrap();
        assert_eq!(entry.journal.notes, "local notes");
        assert_eq!(
            entry.state,
            LocalJournalState::LocalOnly {
                failure: Some(JournalUploadFailure {
                    message: "network error: offline".to_string()
                })
            }
        );
    }
}
