use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, DeletedJournalState, Dispatcher, IssueState, JournalAction, NoticeAction, NoticeId,
};
use crate::vos::{EntityIdValue, IssueId, JournalId};

/// 退避したJournalの投稿結果に応じたActionを生成するFuture。
pub type StartDeletedJournalUploadFuture =
    Pin<Box<dyn Future<Output = Vec<Action>> + Send + 'static>>;

/// 取得結果から消えて退避したJournalを、新規Journalとして投稿し始める。
///
/// `StartDeletedUpload`はFutureをpollする前に同期的にqueueへ追加する。返却したFutureは
/// notes追加のPUTと、その結果を確認するためのIssue GETを順に行う。元の投稿者や日時は送らない。
///
/// # Panics
///
/// Issueがupload中、対象が未登録またはPending以外、もしくは同じIssueの別Journalが
/// upload中の場合にpanicする。
pub fn start_deleted_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
    original_id: JournalId,
) -> StartDeletedJournalUploadFuture
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
            panic!("cannot start deleted journal upload while issue {issue_id} is uploading");
        }
        let entry = store.get_deleted_journal(issue_id, original_id);
        if !matches!(entry.state, DeletedJournalState::Pending { .. }) {
            panic!("cannot start deleted journal upload unless it is pending");
        }
        if store.has_uploading_journal(issue_id) {
            panic!(
                "cannot start deleted journal upload while another journal of issue {issue_id} is uploading"
            );
        }
        entry.notes.clone()
    };

    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartDeletedUpload {
            issue_id,
            original_id,
        });

    Box::pin(
        async move { upload_deleted_journal(client.as_ref(), issue_id, original_id, notes).await },
    )
}

// FIXME: PUTと確認GETを1つのFutureで続けて実行し、結果を最後に1つのActionで反映している。
// PUT成功をStoreへ確定してからGETを始められず、確認GETだけを再試行する経路もない。
// リクエストごとに完了Actionを適用して次のリクエストへ進む実行方式を導入して分割する。
async fn upload_deleted_journal<C>(
    client: &C,
    issue_id: IssueId,
    original_id: JournalId,
    notes: String,
) -> Vec<Action>
where
    C: RedmineClient + Send + Sync + 'static,
{
    if let Err(error) = client.update_issue_notes(issue_id, &notes).await {
        let message = error.to_string();
        return vec![
            NoticeAction::Push {
                id: NoticeId::new(),
                message: format!("削除されたJournalの投稿に失敗しました: {message}"),
            }
            .into(),
            JournalAction::FailDeletedUpload {
                issue_id,
                original_id,
                message,
            }
            .into(),
        ];
    }
    let reason = match client.get_issue(issue_id).await {
        Ok(fetched) if fetched.aggregate.issue.id == issue_id => {
            return vec![
                JournalAction::CompleteDeletedUploadWithFetched {
                    original_id,
                    issue: fetched.aggregate,
                    children: fetched.children,
                }
                .into(),
            ];
        }
        // 別Issueのレスポンスを確認結果として扱うと、他Issueのjournalを根拠に完了させてしまうため、
        // 取得失敗と同じ扱いにする。
        Ok(fetched) => format!(
            "requested issue {} but Redmine returned issue {}",
            issue_id.get(),
            fetched.aggregate.issue.id.get()
        ),
        Err(error) => error.to_string(),
    };
    // notesは投稿済みのため、退避データを削除して再試行による二重投稿を防ぐ。
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message: format!(
                "削除されたJournalを投稿しましたが、確認の取得に失敗しました: {reason}"
            ),
        }
        .into(),
        JournalAction::CompleteDeletedUploadWithoutFetch {
            issue_id,
            original_id,
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
        Category, IssueStatus, Journal, Priority, Project, ProjectIssuesPage, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{IssueAction, JournalUploadFailure};
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::{IssueStatusId, ProjectId};

    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);
    const ORIGINAL_ID: JournalId = JournalId::new(10);

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
            let mut aggregate = sample_issue_aggregate(
                fetched_issue_id,
                "subject",
                IssueStatusId::new(1),
                None,
                None,
                None,
                0,
            );
            aggregate.journals = self.journals.clone();
            Ok(FetchedIssue {
                aggregate,
                children: vec![],
            })
        }

        async fn update_issue(
            &self,
            _: crate::vos::IssueId,
            _: &crate::clients::redmine::IssueUpdate,
        ) -> Result<(), RedmineClientError> {
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

    /// Journal 10を"deleted notes"へ編集した後、取得結果から消えて退避されたdispatcher。
    fn deleted_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.journals = vec![remote_journal(10, "remote")];
        let mut saved = issue.clone();
        saved.journals = vec![];
        for action in [
            Action::from(IssueAction::Sync { issue }),
            JournalAction::EditRemoteNotes {
                issue_id: ISSUE_ID,
                journal_id: ORIGINAL_ID,
                notes: "deleted notes".to_string(),
            }
            .into(),
            IssueAction::UpdateDescription {
                id: ISSUE_ID,
                body: "edited".to_string(),
            }
            .into(),
            IssueAction::Sync { issue: saved }.into(),
        ] {
            dispatcher.dispatch(action);
            dispatcher.consume_action();
        }
        Rc::new(RefCell::new(dispatcher))
    }

    async fn upload_and_apply(
        dispatcher: &Rc<RefCell<Dispatcher>>,
        client: Arc<StubClient>,
    ) -> Vec<Action> {
        let future =
            start_deleted_journal_upload(dispatcher.clone(), client, ISSUE_ID, ORIGINAL_ID);
        dispatcher.borrow_mut().consume_action();
        let actions = future.await;
        actions
    }

    fn apply(dispatcher: &Rc<RefCell<Dispatcher>>, actions: Vec<Action>) {
        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }
    }

    #[test]
    fn start_dispatches_before_the_future_is_polled() {
        let dispatcher = deleted_dispatcher();
        let client = Arc::new(StubClient::succeeds());

        let _future =
            start_deleted_journal_upload(dispatcher.clone(), client.clone(), ISSUE_ID, ORIGINAL_ID);

        assert!(client.request.lock().unwrap().is_none());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_deleted_journal(ISSUE_ID, ORIGINAL_ID)
                .state,
            DeletedJournalState::Uploading
        );
    }

    #[tokio::test]
    async fn put_success_posts_the_notes_as_a_new_journal_and_removes_only_the_target() {
        let dispatcher = deleted_dispatcher();
        let client = Arc::new(StubClient::succeeds_with_journals(vec![remote_journal(
            11,
            "deleted notes",
        )]));

        let actions = upload_and_apply(&dispatcher, client.clone()).await;
        apply(&dispatcher, actions);

        assert_eq!(
            *client.request.lock().unwrap(),
            Some((ISSUE_ID, "deleted notes".to_string()))
        );
        let dispatcher = dispatcher.borrow();
        assert!(dispatcher.store().get_deleted_journals(ISSUE_ID).is_empty());
        let journals = dispatcher.store().get_remote_journals(ISSUE_ID);
        assert_eq!(journals.len(), 1);
        assert_eq!(journals[0].journal.id, JournalId::new(11));
    }

    #[tokio::test]
    async fn put_failure_keeps_the_notes_with_the_failure_and_does_not_fetch() {
        let dispatcher = deleted_dispatcher();
        let client = Arc::new(StubClient::fails());

        let actions = upload_and_apply(&dispatcher, client.clone()).await;
        apply(&dispatcher, actions);

        assert!(client.get_requests.lock().unwrap().is_empty());
        let dispatcher = dispatcher.borrow();
        let entry = dispatcher
            .store()
            .get_deleted_journal(ISSUE_ID, ORIGINAL_ID);
        assert_eq!(entry.notes, "deleted notes");
        assert_eq!(
            entry.state,
            DeletedJournalState::Pending {
                failure: Some(JournalUploadFailure {
                    message: "network error: offline".to_string(),
                }),
            }
        );
        assert_eq!(
            dispatcher.store().get_notices()[0].message,
            "削除されたJournalの投稿に失敗しました: network error: offline"
        );
    }

    #[tokio::test]
    async fn get_failure_removes_the_target_and_tells_that_the_notes_were_posted() {
        let dispatcher = deleted_dispatcher();
        let client = Arc::new(StubClient::fails_to_get());

        let actions = upload_and_apply(&dispatcher, client).await;
        apply(&dispatcher, actions);

        let dispatcher = dispatcher.borrow();
        assert!(dispatcher.store().get_deleted_journals(ISSUE_ID).is_empty());
        assert_eq!(
            dispatcher.store().get_notices()[0].message,
            "削除されたJournalを投稿しましたが、確認の取得に失敗しました: network error: offline"
        );
    }

    #[tokio::test]
    async fn a_fetched_issue_id_mismatch_is_treated_as_a_confirmation_failure() {
        let dispatcher = deleted_dispatcher();
        let client = Arc::new(StubClient::succeeds_with_fetched_issue_id(99, vec![]));

        let actions = upload_and_apply(&dispatcher, client).await;
        apply(&dispatcher, actions);

        let dispatcher = dispatcher.borrow();
        assert!(dispatcher.store().get_deleted_journals(ISSUE_ID).is_empty());
        assert_eq!(
            dispatcher.store().get_notices()[0].message,
            "削除されたJournalを投稿しましたが、確認の取得に失敗しました: requested issue 1 but Redmine returned issue 99"
        );
    }

    #[test]
    #[should_panic(
        expected = "cannot start deleted journal upload while another journal of issue 1 is uploading"
    )]
    fn start_panics_while_the_local_journal_is_uploading() {
        let dispatcher = deleted_dispatcher();
        {
            let mut dispatcher = dispatcher.borrow_mut();
            dispatcher.dispatch(JournalAction::CreateLocal { issue_id: ISSUE_ID });
            dispatcher.consume_action();
            dispatcher.dispatch(JournalAction::StartLocalUpload { issue_id: ISSUE_ID });
            dispatcher.consume_action();
        }

        let _ = start_deleted_journal_upload(
            dispatcher,
            Arc::new(StubClient::succeeds()),
            ISSUE_ID,
            ORIGINAL_ID,
        );
    }
}
