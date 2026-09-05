use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::stores::Action;

pub async fn load_initial_entities<C: RedmineClient>(
    client: &C,
) -> Result<Vec<Action>, RedmineClientError> {
    let (
        users,
        issue_statuses,
        priorities,
        projects,
        trackers,
        target_versions,
        categories,
        time_entity_activities,
    ) = tokio::join!(
        load_users(client),
        load_issue_statuses(client),
        load_priorities(client),
        load_projects(client),
        load_trackers(client),
        load_target_versions(client),
        load_categories(client),
        load_time_entity_activities(client),
    );

    let actions = vec![
        users?,
        issue_statuses?,
        priorities?,
        projects?,
        trackers?,
        target_versions?,
        categories?,
        time_entity_activities?,
    ];

    Ok(actions)
}

async fn load_users<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncUsers {
        users: client.get_users().await?,
    })
}

async fn load_issue_statuses<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncIssueStatuses {
        issue_statuses: client.get_issue_statuses().await?,
    })
}

async fn load_priorities<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncPriorities {
        priorities: client.get_priorities().await?,
    })
}

async fn load_projects<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncProjects {
        projects: client.get_projects().await?,
    })
}

async fn load_trackers<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncTrackers {
        trackers: client.get_trackers().await?,
    })
}

async fn load_target_versions<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncTargetVersions {
        target_versions: client.get_target_versions().await?,
    })
}

async fn load_categories<C: RedmineClient>(client: &C) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncCategories {
        categories: client.get_categories().await?,
    })
}

async fn load_time_entity_activities<C: RedmineClient>(
    client: &C,
) -> Result<Action, RedmineClientError> {
    Ok(Action::SyncTimeEntityActivities {
        time_entity_activities: client.get_time_entity_activities().await?,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::future::Future;
    use std::sync::{Arc, Mutex};

    use tokio::runtime::Builder as TokioRuntimeBuilder;
    use tokio::sync::Barrier;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::load_initial_entities;
    use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::stores::Action;
    use crate::vos::{
        CategoryId, EntityIdValue, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId,
        TimeEntityActivityId, TrackerId, UserId,
    };

    #[test]
    fn load_initial_entities_loads_startup_entities_in_parallel() {
        block_on(async {
            let client = RecordingClient::new(8);

            let actions = tokio::time::timeout(
                std::time::Duration::from_millis(200),
                load_initial_entities(&client),
            )
            .await
            .expect("initial entity usecases did not run in parallel")
            .unwrap();

            assert_eq!(client.started(), expected_started_requests());
            assert_initial_load_actions(&actions);
        });
    }

    #[test]
    fn load_initial_entities_returns_client_error_for_invalid_url() {
        let client = DefaultRedmineClient::new("http://[::1", "secret-token");

        let error = expect_error(block_on(load_initial_entities(&client)));

        match error {
            RedmineClientError::Client { reason } => {
                assert!(!reason.is_empty(), "client error reason should be set");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn load_initial_entities_returns_unauthorized_for_invalid_access_token() {
        let mock_server = block_on(MockServer::start());
        block_on(
            Mock::given(method("GET"))
                .and(header("X-Redmine-API-Key", "invalid-token"))
                .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"failed"}"#))
                .mount(&mock_server),
        );
        let client = DefaultRedmineClient::new(mock_server.uri(), "invalid-token");

        let error = expect_error(block_on(load_initial_entities(&client)));

        match error {
            RedmineClientError::Unauthorized { context } => {
                assert_eq!(context.method, "GET");
                assert_eq!(context.status_code, 401);
                assert!(
                    context.url.starts_with(&mock_server.uri()),
                    "unexpected url: {}",
                    context.url
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn expect_error(result: Result<Vec<Action>, RedmineClientError>) -> RedmineClientError {
        match result {
            Ok(_) => panic!("initial entity load succeeded"),
            Err(error) => error,
        }
    }

    fn expected_started_requests() -> HashSet<String> {
        [
            "users",
            "issue_statuses",
            "priorities",
            "projects",
            "trackers",
            "target_versions",
            "categories",
            "time_entity_activities",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    fn assert_initial_load_actions(actions: &[Action]) {
        assert_eq!(actions.len(), 8);
        let action_names = actions.iter().map(action_name).collect::<HashSet<_>>();
        let expected_action_names = [
            "SyncUsers",
            "SyncIssueStatuses",
            "SyncPriorities",
            "SyncProjects",
            "SyncTrackers",
            "SyncTargetVersions",
            "SyncCategories",
            "SyncTimeEntityActivities",
        ]
        .into_iter()
        .collect::<HashSet<_>>();

        assert_eq!(action_names, expected_action_names);
    }

    fn action_name(action: &Action) -> &'static str {
        match action {
            Action::SyncUsers { .. } => "SyncUsers",
            Action::SyncIssueStatuses { .. } => "SyncIssueStatuses",
            Action::SyncPriorities { .. } => "SyncPriorities",
            Action::SyncProjects { .. } => "SyncProjects",
            Action::SyncTrackers { .. } => "SyncTrackers",
            Action::SyncTargetVersions { .. } => "SyncTargetVersions",
            Action::SyncCategories { .. } => "SyncCategories",
            Action::SyncTimeEntityActivities { .. } => "SyncTimeEntityActivities",
            _ => "Unexpected",
        }
    }

    struct RecordingClient {
        started: Arc<Mutex<Vec<String>>>,
        barrier: Arc<Barrier>,
    }

    impl RecordingClient {
        fn new(expected_request_count: usize) -> Self {
            Self {
                started: Arc::new(Mutex::new(Vec::new())),
                barrier: Arc::new(Barrier::new(expected_request_count)),
            }
        }

        fn started(&self) -> HashSet<String> {
            self.started.lock().unwrap().iter().cloned().collect()
        }

        async fn record(&self, name: impl Into<String>) {
            self.started.lock().unwrap().push(name.into());
            self.barrier.wait().await;
        }
    }

    impl RedmineClient for RecordingClient {
        async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
            self.record("categories").await;
            Ok(vec![Category {
                id: CategoryId::new(1),
                name: "category".to_string(),
            }])
        }

        async fn get_issue(
            &self,
            id: IssueId,
        ) -> Result<(IssueAggregate, Vec<crate::entities::Journal>), RedmineClientError> {
            panic!("initial entity load should not load issue {}", id.get());
        }

        async fn update_issue(&self, _: &IssueAggregate) -> Result<(), RedmineClientError> {
            panic!("initial entity load should not update issue");
        }

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            panic!("initial entity load should not update journal notes");
        }

        async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError> {
            self.record("issue_statuses").await;
            Ok(vec![IssueStatus {
                id: IssueStatusId::new(1),
                name: "status".to_string(),
                is_closed: false,
            }])
        }

        async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError> {
            self.record("priorities").await;
            Ok(vec![Priority {
                id: PriorityId::new(1),
                name: "priority".to_string(),
            }])
        }

        async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError> {
            self.record("projects").await;
            Ok(vec![Project {
                id: ProjectId::new(1),
                name: "project".to_string(),
            }])
        }

        async fn get_project_issues(
            &self,
            _: crate::vos::ProjectId,
            _: std::num::NonZeroUsize,
        ) -> Result<crate::entities::ProjectIssuesPage, RedmineClientError> {
            panic!("initial entity load should not load project issues");
        }

        async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError> {
            self.record("target_versions").await;
            Ok(vec![TargetVersion {
                id: TargetVersionId::new(1),
                name: "version".to_string(),
            }])
        }

        async fn get_time_entity_activities(
            &self,
        ) -> Result<Vec<TimeEntityActivity>, RedmineClientError> {
            self.record("time_entity_activities").await;
            Ok(vec![TimeEntityActivity {
                id: TimeEntityActivityId::new(1),
                name: "activity".to_string(),
                is_default: true,
            }])
        }

        async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError> {
            self.record("trackers").await;
            Ok(vec![Tracker {
                id: TrackerId::new(1),
                name: "tracker".to_string(),
            }])
        }

        async fn get_users(&self) -> Result<Vec<User>, RedmineClientError> {
            self.record("users").await;
            Ok(vec![User {
                id: UserId::new(1),
                name: "user".to_string(),
            }])
        }
    }
}
