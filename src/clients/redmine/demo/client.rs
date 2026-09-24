use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use crate::clients::redmine::base::{FetchedIssue, PROJECT_ISSUES_PAGE_LIMIT, RedmineHttpError};
use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::entities::{
    Category, IssueAggregate, IssueStatus, Journal, Priority, Project, ProjectIssuesPage,
    TargetVersion, TimeEntityActivity, Tracker, User,
};
use crate::vos::{EntityIdValue, IssueId, JournalId, ProjectId};

use super::fixture::DemoFixtureState;

#[derive(Clone)]
pub struct DemoRedmineClient {
    state: Arc<Mutex<DemoFixtureState>>,
}

impl DemoRedmineClient {
    // 呼び出し側のエラー処理を実HTTPクライアントと揃えるため、404のcontextを付ける。
    fn not_found(method: &str, url: String) -> RedmineClientError {
        RedmineClientError::NotFound {
            context: RedmineHttpError {
                method: method.to_string(),
                url,
                status_code: 404,
                response_body: String::new(),
            },
        }
    }

    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DemoFixtureState::initial())),
        }
    }

    fn read<T: Clone>(&self, select: fn(&DemoFixtureState) -> &Vec<T>) -> Vec<T> {
        // Futureを返す前に値を複製し、非同期処理中にlockを保持しない。
        let state = self.state.lock().expect("demo fixture state lock poisoned");
        select(&state).clone()
    }
}

impl Default for DemoRedmineClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RedmineClient for DemoRedmineClient {
    fn get_categories(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Category>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.categories);
        async move { Ok(value) }
    }

    fn get_issue(
        &self,
        id: IssueId,
    ) -> impl std::future::Future<Output = Result<FetchedIssue, RedmineClientError>> + Send {
        let snapshot = {
            let state = self.state.lock().expect("demo fixture state lock poisoned");
            state.issues.get(&id).cloned().map(|aggregate| {
                let journals = state.journals.get(&id).cloned().unwrap_or_default();
                (aggregate, journals)
            })
        };
        async move {
            let Some((aggregate, journals)) = snapshot else {
                return Err(Self::not_found("GET", format!("/issues/{}.json", id.get())));
            };
            Ok(FetchedIssue {
                aggregate,
                journals,
            })
        }
    }

    fn update_issue(
        &self,
        issue: &IssueAggregate,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
        // デモでは渡されたaggregateだけを保存し、実Redmineの更新時に付く履歴や更新日時は生成しない。
        let result = {
            let mut state = self.state.lock().expect("demo fixture state lock poisoned");
            match state.issues.get_mut(&issue.issue.id) {
                Some(stored) => {
                    *stored = issue.clone();
                    Ok(())
                }
                None => Err(Self::not_found(
                    "PUT",
                    format!("/issues/{}.json", issue.issue.id.get()),
                )),
            }
        };
        async move { result }
    }

    fn update_journal_notes(
        &self,
        journal_id: JournalId,
        notes: &str,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
        let result = {
            let mut state = self.state.lock().expect("demo fixture state lock poisoned");
            state
                .journals
                .values_mut()
                .flat_map(|journals| journals.iter_mut())
                .find(|journal| journal.id == journal_id)
                .map(|journal| journal.notes = notes.to_string())
                .ok_or_else(|| {
                    Self::not_found("PUT", format!("/journals/{}.json", journal_id.get()))
                })
        };
        async move { result }
    }

    fn update_issue_notes(
        &self,
        issue_id: IssueId,
        notes: &str,
    ) -> impl std::future::Future<Output = Result<(), RedmineClientError>> + Send {
        let result = {
            let mut state = self.state.lock().expect("demo fixture state lock poisoned");
            if !state.issues.contains_key(&issue_id) {
                Err(Self::not_found(
                    "PUT",
                    format!("/issues/{}.json", issue_id.get()),
                ))
            } else {
                let next_id = state
                    .journals
                    .values()
                    .flatten()
                    .map(|journal| journal.id.get())
                    .max()
                    .unwrap_or(0)
                    + 1;
                let user = state
                    .users
                    .first()
                    .expect("demo fixture has no users")
                    .name
                    .clone();
                state.journals.entry(issue_id).or_default().push(Journal {
                    id: JournalId::new(next_id),
                    issue_id,
                    user,
                    updated_on: Some(chrono::Local::now()),
                    details: Vec::new(),
                    notes: notes.to_string(),
                });
                Ok(())
            }
        };
        async move { result }
    }

    fn get_issue_statuses(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<IssueStatus>, RedmineClientError>> + Send
    {
        let value = self.read(|state| &state.issue_statuses);
        async move { Ok(value) }
    }

    fn get_priorities(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Priority>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.priorities);
        async move { Ok(value) }
    }

    fn get_projects(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Project>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.projects);
        async move { Ok(value) }
    }

    fn get_project_issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> impl std::future::Future<Output = Result<ProjectIssuesPage, RedmineClientError>> + Send
    {
        let offset = page
            .get()
            .checked_sub(1)
            .and_then(|value| value.checked_mul(PROJECT_ISSUES_PAGE_LIMIT))
            .ok_or_else(|| RedmineClientError::Client {
                reason: format!("project issue page {} has an invalid offset", page.get()),
            });
        let result = offset.map(|offset| {
            let mut issues: Vec<_> = {
                let state = self.state.lock().expect("demo fixture state lock poisoned");
                state
                    .issues
                    .values()
                    .filter(|aggregate| aggregate.issue.project_id == project_id)
                    .map(|aggregate| aggregate.issue.clone())
                    .collect()
            };
            issues.sort_by_key(|issue| std::cmp::Reverse(issue.id));
            let total_count = issues.len();
            let issues = issues
                .into_iter()
                .skip(offset)
                .take(PROJECT_ISSUES_PAGE_LIMIT)
                .collect();
            ProjectIssuesPage {
                issues,
                total_count,
                offset,
                limit: PROJECT_ISSUES_PAGE_LIMIT,
            }
        });
        async move { result }
    }

    fn get_target_versions(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TargetVersion>, RedmineClientError>> + Send
    {
        let value = self.read(|state| &state.target_versions);
        async move { Ok(value) }
    }

    fn get_time_entity_activities(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TimeEntityActivity>, RedmineClientError>> + Send
    {
        let value = self.read(|state| &state.time_entity_activities);
        async move { Ok(value) }
    }

    fn get_trackers(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Tracker>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.trackers);
        async move { Ok(value) }
    }

    fn get_users(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<User>, RedmineClientError>> + Send {
        let value = self.read(|state| &state.users);
        async move { Ok(value) }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::num::NonZeroUsize;
    use std::rc::Rc;
    use std::sync::Arc;

    use crate::clients::redmine::demo::DemoRedmineClient;
    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::stores::{Action, Dispatcher, IssueAction, JournalAction};
    use crate::usecases::redmine::{
        start_local_journal_upload, start_remote_journal_upload, upload_issue_action,
    };
    use crate::vos::issue_property_diff::IssueSubjectDiff;
    use crate::vos::{EntityIdValue, IssueId, IssuePropertyDiff, JournalId, ProjectId};
    use tokio::runtime::Builder;

    #[test]
    fn project_issue_pages_filter_sort_and_report_metadata() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();
        let first_page = runtime
            .block_on(client.get_project_issues(ProjectId::new(1), NonZeroUsize::new(1).unwrap()))
            .unwrap();
        assert_eq!(
            first_page
                .issues
                .iter()
                .map(|issue| issue.id.get())
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert!(
            first_page
                .issues
                .iter()
                .all(|issue| issue.project_id == ProjectId::new(1))
        );
        assert_eq!(first_page.total_count, 3);
        assert_eq!(first_page.offset, 0);
        assert_eq!(first_page.limit, 50);

        let other_project = runtime
            .block_on(client.get_project_issues(ProjectId::new(2), NonZeroUsize::new(1).unwrap()))
            .unwrap();
        assert!(other_project.issues.is_empty());
        assert_eq!(other_project.total_count, 0);

        let outside_range = runtime
            .block_on(client.get_project_issues(ProjectId::new(1), NonZeroUsize::new(2).unwrap()))
            .unwrap();
        assert!(outside_range.issues.is_empty());
        assert_eq!(outside_range.total_count, 3);
        assert_eq!(outside_range.offset, 50);
    }

    #[test]
    fn master_getters_return_embedded_fixture_values() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();

        macro_rules! names {
            ($values:expr) => {
                $values
                    .into_iter()
                    .map(|value| value.name)
                    .collect::<Vec<_>>()
            };
        }

        let categories = names!(runtime.block_on(client.get_categories()).unwrap());
        assert_eq!(categories.len(), 1);
        assert!(categories.contains(&String::from("category1")));

        let statuses = names!(runtime.block_on(client.get_issue_statuses()).unwrap());
        assert_eq!(statuses.len(), 6);
        assert!(statuses.contains(&String::from("新規(new)")));

        let priorities = names!(runtime.block_on(client.get_priorities()).unwrap());
        assert_eq!(priorities.len(), 4);
        assert!(priorities.contains(&String::from("major")));

        let projects = names!(runtime.block_on(client.get_projects()).unwrap());
        assert_eq!(projects.len(), 2);
        assert!(projects.contains(&String::from("Sample Project")));

        let versions = names!(runtime.block_on(client.get_target_versions()).unwrap());
        assert_eq!(versions.len(), 1);
        assert!(versions.contains(&String::from("v1.2.3")));

        let activities = names!(
            runtime
                .block_on(client.get_time_entity_activities())
                .unwrap()
        );
        assert_eq!(activities.len(), 3);
        assert!(activities.contains(&String::from("設計")));

        let trackers = names!(runtime.block_on(client.get_trackers()).unwrap());
        assert_eq!(trackers.len(), 3);
        assert!(trackers.contains(&String::from("Bug")));

        let users = names!(runtime.block_on(client.get_users()).unwrap());
        assert_eq!(users.len(), 2);
        assert!(users.contains(&String::from("user1")));
    }

    #[test]
    fn issue_getter_returns_matching_snapshot_and_not_found() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();

        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        assert_eq!(fetched.aggregate.issue.id.get(), 3);
        assert_eq!(
            fetched
                .journals
                .iter()
                .map(|journal| journal.id.get())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let journal_free = runtime.block_on(client.get_issue(IssueId::new(1))).unwrap();
        assert_eq!(journal_free.aggregate.issue.id.get(), 1);
        assert!(journal_free.journals.is_empty());

        assert!(matches!(
            runtime.block_on(client.get_issue(IssueId::new(999))),
            Err(RedmineClientError::NotFound { .. })
        ));
    }

    #[test]
    fn updates_are_visible_to_subsequent_gets() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();
        let mut issue = runtime
            .block_on(client.get_issue(IssueId::new(3)))
            .unwrap()
            .aggregate;
        let updated_on = issue.updated_on;
        issue.issue.subject = "changed subject".into();
        runtime.block_on(client.update_issue(&issue)).unwrap();
        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        assert_eq!(fetched.aggregate.issue.subject, "changed subject");
        assert_eq!(fetched.aggregate.updated_on, updated_on);
        assert_eq!(fetched.journals.len(), 3);

        runtime
            .block_on(client.update_journal_notes(JournalId::new(2), "remote replacement"))
            .unwrap();
        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        assert_eq!(fetched.journals[1].notes, "remote replacement");

        runtime
            .block_on(client.update_issue_notes(IssueId::new(3), "local addition"))
            .unwrap();
        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        assert_eq!(fetched.journals.len(), 4);
        let added = fetched.journals.last().unwrap();
        assert_eq!(added.id.get(), 4);
        assert_eq!(added.issue_id, IssueId::new(3));
        assert_eq!(added.user, "user1");
        assert_eq!(added.notes, "local addition");
    }

    #[test]
    fn updates_report_not_found() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();
        let mut issue = runtime
            .block_on(client.get_issue(IssueId::new(3)))
            .unwrap()
            .aggregate;
        issue.issue.id = IssueId::new(999);
        for result in [
            runtime.block_on(client.update_issue(&issue)),
            runtime.block_on(client.update_journal_notes(JournalId::new(999), "missing")),
            runtime.block_on(client.update_issue_notes(IssueId::new(999), "missing")),
        ] {
            assert!(matches!(result, Err(RedmineClientError::NotFound { .. })));
        }
    }

    #[test]
    fn issue_upload_action_succeeds_and_detects_conflicts() {
        let client = DemoRedmineClient::new();
        let runtime = Builder::new_current_thread().build().unwrap();
        let original = runtime
            .block_on(client.get_issue(IssueId::new(3)))
            .unwrap()
            .aggregate
            .issue
            .subject;
        let diff = IssuePropertyDiff::Subject(IssueSubjectDiff {
            before: original.clone(),
            after: "local edit".into(),
        });
        let actions = runtime.block_on(upload_issue_action(
            &client,
            IssueId::new(3),
            &[diff.clone()],
        ));
        assert!(matches!(
            actions.as_slice(),
            [Action::Issue(IssueAction::Sync { .. })]
        ));
        assert_eq!(
            runtime
                .block_on(client.get_issue(IssueId::new(3)))
                .unwrap()
                .aggregate
                .issue
                .subject,
            "local edit"
        );

        let client = DemoRedmineClient::new();
        let mut server = runtime
            .block_on(client.get_issue(IssueId::new(3)))
            .unwrap()
            .aggregate;
        server.issue.subject = "server edit".into();
        runtime.block_on(client.update_issue(&server)).unwrap();
        let actions = runtime.block_on(upload_issue_action(&client, IssueId::new(3), &[diff]));
        assert!(matches!(
            actions.as_slice(),
            [Action::Issue(IssueAction::UploadConflictsDetected { .. })]
        ));
    }

    fn journal_dispatcher(
        client: &DemoRedmineClient,
        runtime: &tokio::runtime::Runtime,
    ) -> Rc<RefCell<Dispatcher>> {
        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(IssueAction::Sync {
            issue: fetched.aggregate,
        });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::SyncFetched {
            issue_id: IssueId::new(3),
            journals: fetched.journals,
        });
        dispatcher.consume_action();
        Rc::new(RefCell::new(dispatcher))
    }

    #[test]
    fn local_journal_upload_adds_a_journal() {
        let client = Arc::new(DemoRedmineClient::new());
        let runtime = Builder::new_current_thread().build().unwrap();
        let dispatcher = journal_dispatcher(&client, &runtime);
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::CreateLocal {
                issue_id: IssueId::new(3),
            });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::EditLocalNotes {
                issue_id: IssueId::new(3),
                notes: "uploaded local".into(),
            });
        dispatcher.borrow_mut().consume_action();
        let actions = runtime.block_on(start_local_journal_upload(
            dispatcher.clone(),
            client.clone(),
            IssueId::new(3),
        ));
        assert!(matches!(
            actions.as_slice(),
            [Action::Journal(
                JournalAction::CompleteLocalUploadWithFetched { .. }
            )]
        ));
        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        assert_eq!(fetched.journals.last().unwrap().notes, "uploaded local");
    }

    #[test]
    fn remote_journal_upload_succeeds_and_detects_conflict() {
        let runtime = Builder::new_current_thread().build().unwrap();
        let client = Arc::new(DemoRedmineClient::new());
        let dispatcher = journal_dispatcher(&client, &runtime);
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::EditRemoteNotes {
                issue_id: IssueId::new(3),
                journal_id: JournalId::new(2),
                notes: "uploaded remote".into(),
            });
        dispatcher.borrow_mut().consume_action();
        let actions = runtime.block_on(start_remote_journal_upload(
            dispatcher,
            client.clone(),
            IssueId::new(3),
            JournalId::new(2),
        ));
        assert!(matches!(
            actions.as_slice(),
            [Action::Journal(JournalAction::CompleteRemoteUpload { .. })]
        ));
        let fetched = runtime.block_on(client.get_issue(IssueId::new(3))).unwrap();
        assert_eq!(fetched.journals[1].notes, "uploaded remote");

        let client = Arc::new(DemoRedmineClient::new());
        let dispatcher = journal_dispatcher(&client, &runtime);
        dispatcher
            .borrow_mut()
            .dispatch(JournalAction::EditRemoteNotes {
                issue_id: IssueId::new(3),
                journal_id: JournalId::new(2),
                notes: "uploaded remote".into(),
            });
        dispatcher.borrow_mut().consume_action();
        runtime
            .block_on(client.update_journal_notes(JournalId::new(2), "server edit"))
            .unwrap();
        let actions = runtime.block_on(start_remote_journal_upload(
            dispatcher,
            client,
            IssueId::new(3),
            JournalId::new(2),
        ));
        assert!(matches!(
            actions.as_slice(),
            [Action::Journal(
                JournalAction::DetectRemoteUploadConflict { .. }
            )]
        ));
    }
}
