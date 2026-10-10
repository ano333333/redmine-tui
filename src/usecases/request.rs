use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::Dispatcher;
use crate::usecases::UsecaseTask;
use crate::usecases::redmine::{
    continue_remote_journal_upload, fetch_issue, fetch_project_issues_page,
    start_deleted_journal_upload, start_issue_upload, start_local_journal_upload,
    start_remote_journal_upload, upload_issue_action,
};
use crate::vos::{IssueId, IssuePropertyDiff, JournalId, ProjectId};

/// runnerに起動を依頼するRedmine Usecase。1つのvariantが1つのUsecaseに対応する。
#[derive(Clone, Debug, PartialEq)]
pub enum UsecaseRequest {
    FetchIssue {
        id: IssueId,
    },
    FetchProjectIssuesPage {
        project_id: ProjectId,
        page: NonZeroUsize,
    },
    StartIssueUpload {
        id: IssueId,
    },
    /// 競合解決後のIssue保存を`retry_diffs`で再開する。
    ///
    /// `retry_diffs`はStoreに保存されないため、競合解決の同期処理で作った値を要求に載せて運ぶ。
    ContinueIssueUpload {
        id: IssueId,
        retry_diffs: Vec<IssuePropertyDiff>,
    },
    StartRemoteJournalUpload {
        issue_id: IssueId,
        journal_id: JournalId,
    },
    ContinueRemoteJournalUpload {
        issue_id: IssueId,
        journal_id: JournalId,
        resolved_notes: String,
    },
    StartLocalJournalUpload {
        issue_id: IssueId,
    },
    StartDeletedJournalUpload {
        issue_id: IssueId,
        original_id: JournalId,
    },
}

/// `request`に対応するUsecaseを起動し、非同期の処理があればそのtaskを返す。
///
/// 起動時の同期Actionは`dispatcher`へ積むだけで消費しない。
pub fn start_usecase<C>(
    request: UsecaseRequest,
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    match request {
        UsecaseRequest::FetchIssue { id } => fetch_issue(dispatcher, client, id),
        UsecaseRequest::FetchProjectIssuesPage { project_id, page } => {
            fetch_project_issues_page(dispatcher, client, project_id, page)
        }
        UsecaseRequest::StartIssueUpload { id } => start_issue_upload(dispatcher, client, id),
        UsecaseRequest::ContinueIssueUpload { id, retry_diffs } => Some(Box::pin(async move {
            upload_issue_action(client.as_ref(), id, &retry_diffs)
                .await
                .into()
        })),
        UsecaseRequest::StartRemoteJournalUpload {
            issue_id,
            journal_id,
        } => start_remote_journal_upload(dispatcher, client, issue_id, journal_id),
        UsecaseRequest::ContinueRemoteJournalUpload {
            issue_id,
            journal_id,
            resolved_notes,
        } => {
            continue_remote_journal_upload(dispatcher, client, issue_id, journal_id, resolved_notes)
        }
        UsecaseRequest::StartLocalJournalUpload { issue_id } => {
            start_local_journal_upload(dispatcher, client, issue_id)
        }
        UsecaseRequest::StartDeletedJournalUpload {
            issue_id,
            original_id,
        } => start_deleted_journal_upload(dispatcher, client, issue_id, original_id),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::num::NonZeroUsize;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use crate::clients::redmine::base::FetchedIssue;
    use crate::clients::redmine::{IssueUpdate, RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Journal, Priority, Project, ProjectIssuesPage,
        TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::stores::{
        Action, DeletedJournalState, Dispatcher, IssueAction, IssueFetchState, IssueState,
        JournalAction, LocalJournalState, ProjectIssuesPageState, RemoteJournalState,
    };
    use crate::test_support::{complete_usecase, local_datetime, sample_issue_aggregate};
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crate::vos::{IssueId, IssuePropertyDiff, IssueStatusId, JournalId, ProjectId};

    use super::{UsecaseRequest, start_usecase};

    const ISSUE_ID: IssueId = IssueId::new(1);
    const JOURNAL_ID: JournalId = JournalId::new(10);

    /// Issueの取得には`issue`を返し、PUTした内容を記録する。
    struct RecordingClient {
        issue: IssueAggregate,
        issue_updates: Mutex<Vec<IssueUpdate>>,
        journal_notes: Mutex<Vec<String>>,
    }

    impl RecordingClient {
        fn new(issue: IssueAggregate) -> Arc<Self> {
            Arc::new(Self {
                issue,
                issue_updates: Mutex::new(Vec::new()),
                journal_notes: Mutex::new(Vec::new()),
            })
        }
    }

    impl RedmineClient for RecordingClient {
        async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
            Ok(FetchedIssue {
                aggregate: self.issue.clone(),
                children: vec![],
            })
        }

        async fn update_issue(
            &self,
            _: IssueId,
            update: &IssueUpdate,
        ) -> Result<(), RedmineClientError> {
            self.issue_updates.lock().unwrap().push(update.clone());
            Ok(())
        }

        async fn update_journal_notes(
            &self,
            _: JournalId,
            notes: &str,
        ) -> Result<(), RedmineClientError> {
            self.journal_notes.lock().unwrap().push(notes.to_string());
            Ok(())
        }

        async fn update_issue_notes(&self, _: IssueId, _: &str) -> Result<(), RedmineClientError> {
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
            _: NonZeroUsize,
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

    fn issue() -> IssueAggregate {
        sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0)
    }

    fn issue_with_journal_notes(notes: &str) -> IssueAggregate {
        let mut issue = issue();
        issue.journals = vec![Journal {
            id: JOURNAL_ID,
            issue_id: ISSUE_ID,
            user: "alice".to_string(),
            updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
            details: vec![],
            notes: notes.to_string(),
        }];
        issue
    }

    fn dispatcher_with(issue: IssueAggregate, actions: Vec<Action>) -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        crate::test_support::dispatch_loaded_issue(&mut dispatcher, issue);
        for action in actions {
            dispatcher.dispatch(action);
            dispatcher.consume_action();
        }
        Rc::new(RefCell::new(dispatcher))
    }

    /// 要求からUsecaseを起動し、起動時の同期Actionを消費する。起動したかを返す。
    fn start(request: UsecaseRequest, dispatcher: &Rc<RefCell<Dispatcher>>) -> bool {
        let task = start_usecase(request, dispatcher.clone(), RecordingClient::new(issue()));
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        task.is_some()
    }

    #[test]
    fn fetch_issue_starts_fetching() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));

        assert!(start(
            UsecaseRequest::FetchIssue { id: ISSUE_ID },
            &dispatcher
        ));

        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .try_get_issue_fetch_state(ISSUE_ID),
            Some(IssueFetchState::Fetching)
        );
    }

    #[test]
    fn fetch_project_issues_page_starts_loading() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let project_id = ProjectId::new(12);

        assert!(start(
            UsecaseRequest::FetchProjectIssuesPage {
                project_id,
                page: NonZeroUsize::MIN,
            },
            &dispatcher,
        ));

        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(project_id, NonZeroUsize::MIN),
            Some(ProjectIssuesPageState::Loading { .. })
        ));
    }

    #[test]
    fn start_issue_upload_starts_uploading() {
        let dispatcher = dispatcher_with(
            issue(),
            vec![
                IssueAction::UpdateDescription {
                    id: ISSUE_ID,
                    body: "edited".to_string(),
                }
                .into(),
            ],
        );

        assert!(start(
            UsecaseRequest::StartIssueUpload { id: ISSUE_ID },
            &dispatcher
        ));

        assert_eq!(
            dispatcher.borrow().store().try_get_issue_state(ISSUE_ID),
            Some(IssueState::Uploading)
        );
    }

    #[tokio::test]
    async fn continue_issue_upload_puts_the_retry_diffs() {
        let mut server_issue = issue();
        server_issue.issue.description = "server description".to_string();
        let client = RecordingClient::new(server_issue);
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));

        complete_usecase(start_usecase(
            UsecaseRequest::ContinueIssueUpload {
                id: ISSUE_ID,
                retry_diffs: vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
                    before: "server description".to_string(),
                    after: "resolved description".to_string(),
                })],
            },
            dispatcher.clone(),
            client.clone(),
        ))
        .await;

        assert_eq!(
            *client.issue_updates.lock().unwrap(),
            vec![IssueUpdate {
                description: Some("resolved description".to_string()),
                ..IssueUpdate::default()
            }]
        );
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn start_remote_journal_upload_starts_uploading() {
        let dispatcher = dispatcher_with(
            issue_with_journal_notes("remote notes"),
            vec![
                JournalAction::EditRemoteNotes {
                    issue_id: ISSUE_ID,
                    journal_id: JOURNAL_ID,
                    notes: "edited notes".to_string(),
                }
                .into(),
            ],
        );

        assert!(start(
            UsecaseRequest::StartRemoteJournalUpload {
                issue_id: ISSUE_ID,
                journal_id: JOURNAL_ID,
            },
            &dispatcher,
        ));

        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_remote_journal(ISSUE_ID, JOURNAL_ID)
                .state,
            RemoteJournalState::Uploading { .. }
        ));
    }

    #[tokio::test]
    async fn continue_remote_journal_upload_puts_the_resolved_notes() {
        let dispatcher = dispatcher_with(
            issue_with_journal_notes("remote notes"),
            vec![
                JournalAction::EditRemoteNotes {
                    issue_id: ISSUE_ID,
                    journal_id: JOURNAL_ID,
                    notes: "edited notes".to_string(),
                }
                .into(),
                JournalAction::StartRemoteUpload {
                    issue_id: ISSUE_ID,
                    journal_id: JOURNAL_ID,
                }
                .into(),
                JournalAction::DetectRemoteUploadConflict {
                    journal_id: JOURNAL_ID,
                    issue: issue_with_journal_notes("server notes"),
                    children: vec![],
                }
                .into(),
            ],
        );
        let client = RecordingClient::new(issue_with_journal_notes("server notes"));

        complete_usecase(start_usecase(
            UsecaseRequest::ContinueRemoteJournalUpload {
                issue_id: ISSUE_ID,
                journal_id: JOURNAL_ID,
                resolved_notes: "resolved notes".to_string(),
            },
            dispatcher,
            client.clone(),
        ))
        .await;

        assert_eq!(
            *client.journal_notes.lock().unwrap(),
            vec!["resolved notes".to_string()]
        );
    }

    #[test]
    fn start_local_journal_upload_starts_uploading() {
        let dispatcher = dispatcher_with(
            issue(),
            vec![
                JournalAction::CreateLocal { issue_id: ISSUE_ID }.into(),
                JournalAction::EditLocalNotes {
                    issue_id: ISSUE_ID,
                    notes: "local notes".to_string(),
                }
                .into(),
            ],
        );

        assert!(start(
            UsecaseRequest::StartLocalJournalUpload { issue_id: ISSUE_ID },
            &dispatcher,
        ));

        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_local_journal(ISSUE_ID)
                .state,
            LocalJournalState::Uploading
        ));
    }

    #[test]
    fn start_deleted_journal_upload_starts_uploading() {
        let mut saved = issue();
        saved.journals = vec![];
        // 編集したJournalが保存後の取得結果から消え、投稿待ちとして退避された状態を作る。
        let dispatcher = dispatcher_with(
            issue_with_journal_notes("remote notes"),
            vec![
                JournalAction::EditRemoteNotes {
                    issue_id: ISSUE_ID,
                    journal_id: JOURNAL_ID,
                    notes: "deleted notes".to_string(),
                }
                .into(),
                IssueAction::UpdateDescription {
                    id: ISSUE_ID,
                    body: "edited".to_string(),
                }
                .into(),
                IssueAction::StartUpload { id: ISSUE_ID }.into(),
                IssueAction::UploadSucceeded {
                    issue: saved,
                    children: vec![],
                }
                .into(),
            ],
        );

        assert!(start(
            UsecaseRequest::StartDeletedJournalUpload {
                issue_id: ISSUE_ID,
                original_id: JOURNAL_ID,
            },
            &dispatcher,
        ));

        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_deleted_journal(ISSUE_ID, JOURNAL_ID)
                .state,
            DeletedJournalState::Uploading
        ));
    }
}
