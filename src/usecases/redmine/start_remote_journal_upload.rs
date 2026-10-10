use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{
    Action, Dispatcher, IssueState, JournalAction, NoticeAction, NoticeId, RemoteJournalState,
};
use crate::usecases::UsecaseTask;
use crate::vos::{EntityIdValue, IssueId, JournalId};

use super::resolve_remote_journal_upload::{
    RemoteJournalUploadResolution, resolve_remote_journal_upload,
};

/// FIXME: usecaseがUI表示物(notice/toast)の文言を組み立てているのは設計上の負債である。
/// 将来的にはStoreのJournal状態を見て判断するtoast component等を導入し、
/// この処理をそちらへ移すべき。
pub fn remote_journal_upload_failure_actions(
    issue_id: IssueId,
    journal_id: JournalId,
    message: String,
) -> Vec<Action> {
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message: format!("Remote Journalの保存に失敗しました: {message}"),
        }
        .into(),
        JournalAction::FailRemoteUpload {
            issue_id,
            journal_id,
            message,
        }
        .into(),
    ]
}

/// 編集済みのRemote Journalのuploadを開始する。
///
/// `StartRemoteUpload`はFutureをpollする前に同期的にqueueへ追加し、返却したFutureは
/// uploadの完了Actionを返す。
///
/// # Panics
///
/// Issueがupload中、対象JournalがEdited以外、または同じIssueの別Journalがupload中の場合に
/// panicする。
pub fn start_remote_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
    journal_id: JournalId,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let diff = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        if matches!(
            store.try_get_issue_state(issue_id),
            Some(IssueState::Uploading)
        ) {
            panic!("cannot start remote journal upload while issue {issue_id} is uploading");
        }
        let entry = store.get_remote_journal(issue_id, journal_id);
        let diff = match &entry.state {
            RemoteJournalState::Edited { diff, .. } => diff.clone(),
            _ => panic!(
                "cannot start remote journal upload unless remote journal {journal_id} is edited"
            ),
        };
        // 非同期処理を作る前にも検査し、StoreのAction入口での排他検査と合わせて
        // usecaseの直接呼び出しと直接dispatchの両方を拒否する。
        if store.has_uploading_journal(issue_id) {
            panic!(
                "cannot start remote journal upload while another journal of issue {issue_id} is uploading"
            );
        }
        diff
    };

    dispatcher
        .borrow_mut()
        .dispatch(JournalAction::StartRemoteUpload {
            issue_id,
            journal_id,
        });

    Some(Box::pin(async move {
        upload_remote_journal_action(
            client.as_ref(),
            issue_id,
            journal_id,
            &diff.before,
            &diff.after,
        )
        .await
        .into()
    }))
}

/// 保存前の取得、対象JournalのPUT、確認の取得を順に行い、結果のActionを返す。
///
/// `before`を競合判定の基準にして`after`を保存する。保存前の取得から対象が消えていれば、
/// 旧IDへPUTせず`after`を退避する。PUT成功後に確認の取得だけが失敗した場合は、PUTを
/// 繰り返さないよう、保存前の取得値の対象notesを`after`にした値で完了させる。
// FIXME: 保存前の取得・PUT・確認の取得を1つのFutureで続けて実行し、結果を最後に1つのActionで
// 反映している。PUT成功をStoreへ確定してから確認の取得を始められず、確認の取得だけを
// 再試行する経路もない。リクエストごとに完了Actionを適用して次へ進む実行方式で分割する。
pub(super) async fn upload_remote_journal_action<C>(
    client: &C,
    issue_id: IssueId,
    journal_id: JournalId,
    before: &str,
    after: &str,
) -> Vec<Action>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let mut preflight = match client.get_issue(issue_id).await {
        Ok(fetched) if fetched.aggregate.issue.id == issue_id => fetched,
        Ok(fetched) => {
            return remote_journal_upload_failure_actions(
                issue_id,
                journal_id,
                mismatched_issue_message(issue_id, fetched.aggregate.issue.id),
            );
        }
        Err(error) => {
            return remote_journal_upload_failure_actions(issue_id, journal_id, error.to_string());
        }
    };
    let Some(server_notes) = preflight
        .aggregate
        .journals
        .iter()
        .find(|journal| journal.id == journal_id)
        .map(|journal| journal.notes.clone())
    else {
        return vec![
            JournalAction::EvacuateMissingRemoteUpload {
                journal_id,
                notes: after.to_string(),
                issue: preflight.aggregate,
                children: preflight.children,
            }
            .into(),
        ];
    };
    match resolve_remote_journal_upload(before, after, &server_notes) {
        // 同じ編集内容が既にサーバーへ反映されていれば、重複PUTせず正常完了として収束させる。
        RemoteJournalUploadResolution::AlreadyApplied => {
            return vec![
                JournalAction::CompleteRemoteUpload {
                    journal_id,
                    issue: preflight.aggregate,
                    children: preflight.children,
                }
                .into(),
            ];
        }
        RemoteJournalUploadResolution::Conflict => {
            return vec![
                JournalAction::DetectRemoteUploadConflict {
                    journal_id,
                    issue: preflight.aggregate,
                    children: preflight.children,
                }
                .into(),
            ];
        }
        RemoteJournalUploadResolution::Upload => {}
    }
    if let Err(error) = client.update_journal_notes(journal_id, after).await {
        return remote_journal_upload_failure_actions(issue_id, journal_id, error.to_string());
    }

    let reason = match client.get_issue(issue_id).await {
        Ok(confirmed) if confirmed.aggregate.issue.id == issue_id => {
            return vec![
                JournalAction::CompleteRemoteUpload {
                    journal_id,
                    issue: confirmed.aggregate,
                    children: confirmed.children,
                }
                .into(),
            ];
        }
        Ok(confirmed) => mismatched_issue_message(issue_id, confirmed.aggregate.issue.id),
        Err(error) => error.to_string(),
    };
    // notesを空にしてRedmineがJournalを削除した場合も、次に取得するまで一覧に残る。
    preflight
        .aggregate
        .journals
        .iter_mut()
        .find(|journal| journal.id == journal_id)
        .expect("the target was found in the preflight fetch")
        .notes = after.to_string();
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message: format!(
                "Journal #{journal_id}を保存しましたが、確認の取得に失敗しました: {reason}"
            ),
        }
        .into(),
        JournalAction::CompleteRemoteUpload {
            journal_id,
            issue: preflight.aggregate,
            children: preflight.children,
        }
        .into(),
    ]
}

fn mismatched_issue_message(requested: IssueId, returned: IssueId) -> String {
    format!(
        "requested issue {} but Redmine returned issue {}",
        requested.get(),
        returned.get()
    )
}

#[cfg(test)]
pub(super) mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::sync::Mutex;

    use crate::clients::redmine::base::FetchedIssue;
    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueChild, IssueStatus, Journal, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{
        Action, DeletedJournalEntry, DeletedJournalState, Dispatcher, IssueAction, IssueState,
        JournalAction, NoticeAction, RemoteJournalState,
    };
    use crate::test_support::complete_usecase;
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::{IssueId, IssueStatusId, JournalId, TrackerId};

    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);
    const JOURNAL_ID: JournalId = JournalId::new(10);

    pub(in crate::usecases::redmine) fn journal_with_notes(notes: &str) -> Journal {
        Journal {
            id: JOURNAL_ID,
            issue_id: ISSUE_ID,
            user: "alice".to_string(),
            updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
            details: vec![],
            notes: notes.to_string(),
        }
    }

    fn journal() -> Journal {
        journal_with_notes("remote notes")
    }

    /// `journals`を持ち、子一覧の題名が`child_subject`のIssue 1の取得結果。
    pub(in crate::usecases::redmine) fn fetched(
        journals: Vec<Journal>,
        child_subject: &str,
    ) -> FetchedIssue {
        let mut aggregate =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        aggregate.journals = journals;
        FetchedIssue {
            aggregate,
            children: vec![IssueChild {
                id: IssueId::new(2),
                tracker_id: TrackerId::new(1),
                subject: child_subject.to_string(),
                children: vec![],
            }],
        }
    }

    pub(in crate::usecases::redmine) fn offline() -> RedmineClientError {
        RedmineClientError::Network {
            reason: "offline".to_string(),
        }
    }

    fn edited_issue_and_journal(dispatcher: &mut Dispatcher) {
        edited_issue_with_journals(dispatcher, vec![journal()]);
    }

    /// `journals`を持つIssueを登録し、そのうちJOURNAL_IDのJournalを編集する。
    fn edited_issue_with_journals(dispatcher: &mut Dispatcher, journals: Vec<Journal>) {
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.issue.description = "issue body".to_string();
        issue.journals = journals;
        crate::test_support::dispatch_loaded_issue(dispatcher, issue);
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            notes: "edited notes".to_string(),
        }));
        dispatcher.consume_action();
    }

    /// GETの結果を呼び出し順に返し、PUTしたnotesを記録する。
    pub(in crate::usecases::redmine) struct StubClient {
        get_results: Mutex<VecDeque<Result<FetchedIssue, RedmineClientError>>>,
        put_result: Result<(), RedmineClientError>,
        pub(in crate::usecases::redmine) put_notes: Mutex<Vec<String>>,
    }

    impl StubClient {
        pub(in crate::usecases::redmine) fn new(
            get_results: Vec<Result<FetchedIssue, RedmineClientError>>,
            put_result: Result<(), RedmineClientError>,
        ) -> Self {
            Self {
                get_results: Mutex::new(get_results.into()),
                put_result,
                put_notes: Mutex::new(Vec::new()),
            }
        }
    }

    impl RedmineClient for StubClient {
        async fn update_journal_notes(
            &self,
            _: JournalId,
            notes: &str,
        ) -> Result<(), RedmineClientError> {
            self.put_notes.lock().unwrap().push(notes.to_string());
            self.put_result.clone()
        }

        async fn update_issue_notes(&self, _: IssueId, _: &str) -> Result<(), RedmineClientError> {
            unreachable!()
        }

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
            _: &crate::clients::redmine::IssueUpdate,
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
            _: crate::vos::ProjectId,
            _: std::num::NonZeroUsize,
        ) -> Result<crate::entities::ProjectIssuesPage, RedmineClientError> {
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

    fn unused_client() -> Arc<StubClient> {
        Arc::new(StubClient::new(vec![], Ok(())))
    }

    fn assert_panics(dispatcher: &Rc<RefCell<Dispatcher>>) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            start_remote_journal_upload(dispatcher.clone(), unused_client(), ISSUE_ID, JOURNAL_ID);
        }));
        assert!(result.is_err());
    }

    fn dispatch_all(dispatcher: &Rc<RefCell<Dispatcher>>, actions: Vec<Action>) {
        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
        }
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
    }

    #[test]
    fn start_dispatches_start_remote_upload_and_keeps_the_diff() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        let future =
            start_remote_journal_upload(dispatcher.clone(), unused_client(), ISSUE_ID, JOURNAL_ID);
        dispatcher.borrow_mut().consume_action();

        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Uploading { diff, conflict } = &entry.state else {
            panic!("expected uploading state");
        };
        assert_eq!(diff.before, "remote notes");
        assert_eq!(diff.after, "edited notes");
        assert!(conflict.is_none());
        drop(future);
    }

    #[test]
    fn start_panics_when_the_issue_is_uploading() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        dispatcher.dispatch(IssueAction::UpdateDescription {
            id: ISSUE_ID,
            body: "edited issue body".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(IssueAction::StartUpload { id: ISSUE_ID });
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        assert!(matches!(
            dispatcher.borrow().store().try_get_issue_state(ISSUE_ID),
            Some(IssueState::Uploading)
        ));

        assert_panics(&dispatcher);
    }

    #[test]
    fn start_panics_when_the_journal_is_synced() {
        let mut dispatcher = Dispatcher::new();
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.issue.description = "issue body".to_string();
        issue.journals = vec![journal()];
        crate::test_support::dispatch_loaded_issue(&mut dispatcher, issue);
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        assert_panics(&dispatcher);
    }

    #[test]
    fn start_panics_when_another_journal_of_the_issue_is_uploading() {
        let mut dispatcher = Dispatcher::new();
        let mut other = journal();
        other.id = JournalId::new(11);
        other.notes = "other notes".to_string();
        edited_issue_with_journals(&mut dispatcher, vec![journal(), other]);
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JournalId::new(11),
            notes: "other edited notes".to_string(),
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID,
            journal_id: JournalId::new(11),
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        assert_panics(&dispatcher);
    }

    #[test]
    fn start_panics_when_the_local_journal_is_uploading() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        dispatcher.dispatch(JournalAction::CreateLocal { issue_id: ISSUE_ID });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::StartLocalUpload { issue_id: ISSUE_ID });
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));

        assert_panics(&dispatcher);
    }

    #[tokio::test]
    async fn get_failure_restores_the_edited_state_with_the_failure() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = Arc::new(StubClient::new(vec![Err(offline())], Ok(())));

        let actions = complete_usecase(start_remote_journal_upload(
            dispatcher.clone(),
            client,
            ISSUE_ID,
            JOURNAL_ID,
        ))
        .await;

        let [
            Action::Notice(NoticeAction::Push { message, .. }),
            Action::Journal(JournalAction::FailRemoteUpload { .. }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and FailRemoteUpload");
        };
        assert_eq!(
            message,
            "Remote Journalの保存に失敗しました: network error: offline"
        );
        dispatch_all(&dispatcher, actions);
        let dispatcher = dispatcher.borrow();
        let entry = dispatcher.store().get_remote_journal(ISSUE_ID, JOURNAL_ID);
        let RemoteJournalState::Edited { diff, failure } = &entry.state else {
            panic!("expected edited state");
        };
        assert_eq!(diff.before, "remote notes");
        assert_eq!(diff.after, "edited notes");
        assert_eq!(
            failure.as_ref().map(|failure| failure.message.as_str()),
            Some("network error: offline")
        );
    }

    #[tokio::test]
    async fn a_journal_missing_from_the_get_result_is_evacuated_without_a_put() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched(vec![], "fetched"))],
            Ok(()),
        ));

        let actions = complete_usecase(start_remote_journal_upload(
            dispatcher.clone(),
            client.clone(),
            ISSUE_ID,
            JOURNAL_ID,
        ))
        .await;

        assert!(client.put_notes.lock().unwrap().is_empty());
        dispatch_all(&dispatcher, actions);
        let dispatcher = dispatcher.borrow();
        assert!(dispatcher.store().get_remote_journals(ISSUE_ID).is_empty());
        assert_eq!(
            dispatcher.store().get_deleted_journals(ISSUE_ID),
            &[DeletedJournalEntry {
                original_id: JOURNAL_ID,
                notes: "edited notes".to_string(),
                state: DeletedJournalState::Pending { failure: None },
            }]
        );
    }

    #[tokio::test]
    async fn a_mismatched_issue_id_fails_without_a_put() {
        let mut other = fetched(vec![journal()], "fetched");
        other.aggregate.issue.id = IssueId::new(99);
        let client = StubClient::new(vec![Ok(other)], Ok(()));

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            _,
            Action::Journal(JournalAction::FailRemoteUpload { message, .. }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and FailRemoteUpload");
        };
        assert_eq!(message, "requested issue 1 but Redmine returned issue 99");
        assert!(client.put_notes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_missing_journal_returns_the_notes_to_evacuate_with_the_fetched_issue() {
        let client = StubClient::new(vec![Ok(fetched(vec![], "fetched"))], Ok(()));

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            Action::Journal(JournalAction::EvacuateMissingRemoteUpload {
                journal_id,
                notes,
                issue,
                children,
            }),
        ] = actions.as_slice()
        else {
            panic!("expected EvacuateMissingRemoteUpload");
        };
        assert_eq!(*journal_id, JOURNAL_ID);
        assert_eq!(notes, "edited notes");
        assert!(issue.journals.is_empty());
        assert_eq!(children[0].subject, "fetched");
    }

    #[tokio::test]
    async fn server_equal_to_the_edited_notes_completes_with_the_fetched_issue_without_a_put() {
        let client = StubClient::new(
            vec![Ok(fetched(
                vec![journal_with_notes("edited notes")],
                "fetched",
            ))],
            Ok(()),
        );

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            Action::Journal(JournalAction::CompleteRemoteUpload {
                issue, children, ..
            }),
        ] = actions.as_slice()
        else {
            panic!("expected CompleteRemoteUpload");
        };
        assert_eq!(issue.journals[0].notes, "edited notes");
        assert_eq!(children[0].subject, "fetched");
        assert!(client.put_notes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_changed_server_returns_the_conflict_with_the_fetched_issue_without_a_put() {
        let client = StubClient::new(
            vec![Ok(fetched(
                vec![journal_with_notes("server notes")],
                "fetched",
            ))],
            Ok(()),
        );

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            Action::Journal(JournalAction::DetectRemoteUploadConflict {
                journal_id,
                issue,
                children,
            }),
        ] = actions.as_slice()
        else {
            panic!("expected DetectRemoteUploadConflict");
        };
        assert_eq!(*journal_id, JOURNAL_ID);
        assert_eq!(issue.journals[0].notes, "server notes");
        assert_eq!(children[0].subject, "fetched");
        assert!(client.put_notes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_unchanged_server_puts_once_and_completes_with_the_confirmed_issue() {
        let client = StubClient::new(
            vec![
                Ok(fetched(vec![journal()], "before put")),
                Ok(fetched(
                    vec![journal_with_notes("confirmed notes")],
                    "after put",
                )),
            ],
            Ok(()),
        );

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            Action::Journal(JournalAction::CompleteRemoteUpload {
                issue, children, ..
            }),
        ] = actions.as_slice()
        else {
            panic!("expected CompleteRemoteUpload");
        };
        assert_eq!(issue.journals[0].notes, "confirmed notes");
        assert_eq!(children[0].subject, "after put");
        assert_eq!(*client.put_notes.lock().unwrap(), vec!["edited notes"]);
    }

    #[tokio::test]
    async fn a_failed_put_fails_without_a_confirmation() {
        let client = StubClient::new(
            vec![Ok(fetched(vec![journal()], "fetched"))],
            Err(RedmineClientError::Network {
                reason: "put failed".to_string(),
            }),
        );

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            _,
            Action::Journal(JournalAction::FailRemoteUpload { message, .. }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and FailRemoteUpload");
        };
        assert_eq!(message, "network error: put failed");
    }

    #[tokio::test]
    async fn a_confirmation_failure_completes_with_the_sent_notes_without_putting_again() {
        let client = StubClient::new(
            vec![Ok(fetched(vec![journal()], "before put")), Err(offline())],
            Ok(()),
        );

        let actions = upload_remote_journal_action(
            &client,
            ISSUE_ID,
            JOURNAL_ID,
            "remote notes",
            "edited notes",
        )
        .await;

        let [
            Action::Notice(NoticeAction::Push { message, .. }),
            Action::Journal(JournalAction::CompleteRemoteUpload {
                issue, children, ..
            }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and CompleteRemoteUpload");
        };
        assert_eq!(
            message,
            "Journal #10を保存しましたが、確認の取得に失敗しました: network error: offline"
        );
        assert_eq!(issue.journals[0].notes, "edited notes");
        assert_eq!(children[0].subject, "before put");
        assert_eq!(*client.put_notes.lock().unwrap(), vec!["edited notes"]);
    }

    #[tokio::test]
    async fn emptied_notes_removed_by_redmine_leave_the_journal_list() {
        let mut dispatcher = Dispatcher::new();
        edited_issue_and_journal(&mut dispatcher);
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            notes: String::new(),
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = Arc::new(StubClient::new(
            vec![
                Ok(fetched(vec![journal()], "fetched")),
                Ok(fetched(vec![], "fetched")),
            ],
            Ok(()),
        ));

        let actions = complete_usecase(start_remote_journal_upload(
            dispatcher.clone(),
            client.clone(),
            ISSUE_ID,
            JOURNAL_ID,
        ))
        .await;

        assert_eq!(*client.put_notes.lock().unwrap(), vec![""]);
        dispatch_all(&dispatcher, actions);
        let dispatcher = dispatcher.borrow();
        assert!(dispatcher.store().get_remote_journals(ISSUE_ID).is_empty());
        assert!(dispatcher.store().get_deleted_journals(ISSUE_ID).is_empty());
    }
}
