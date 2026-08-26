use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::entities::Issue;

/// 競合がないことを確認済みの Issue を Redmine サーバーへアップロードする。
pub async fn upload_issue(
    client: &impl RedmineClient,
    issue: &Issue,
) -> Result<(), RedmineClientError> {
    client.update_issue(issue).await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, Issue, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity,
        Tracker, User,
    };
    use crate::test_support::sample_issue;
    use crate::vos::{IssueId, IssueStatusId};

    use super::upload_issue;

    #[tokio::test]
    async fn uploads_the_supplied_issue() {
        let client = StubClient::succeeds();
        let issue = sample_issue(
            7,
            "merged subject",
            IssueStatusId::new(2),
            None,
            None,
            None,
            0,
        );

        upload_issue(&client, &issue).await.unwrap();

        let uploaded = client.uploaded.lock().unwrap();
        assert_eq!(uploaded.len(), 1);
        assert_eq!(uploaded[0].id, issue.id);
        assert_eq!(uploaded[0].subject, issue.subject);
    }

    #[tokio::test]
    async fn propagates_client_error() {
        let expected = RedmineClientError::Network {
            reason: "offline".to_string(),
        };
        let client = StubClient::fails(expected.clone());
        let issue = sample_issue(7, "subject", IssueStatusId::new(1), None, None, None, 0);

        let actual = upload_issue(&client, &issue).await.unwrap_err();

        assert_eq!(actual, expected);
    }

    struct StubClient {
        uploaded: Mutex<Vec<Issue>>,
        error: Option<RedmineClientError>,
    }

    impl StubClient {
        fn succeeds() -> Self {
            Self {
                uploaded: Mutex::new(Vec::new()),
                error: None,
            }
        }

        fn fails(error: RedmineClientError) -> Self {
            Self {
                uploaded: Mutex::new(Vec::new()),
                error: Some(error),
            }
        }
    }

    impl RedmineClient for StubClient {
        async fn update_issue(&self, issue: &Issue) -> Result<(), RedmineClientError> {
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            self.uploaded.lock().unwrap().push(issue.clone());
            Ok(())
        }

        async fn get_issue(&self, _: IssueId) -> Result<Issue, RedmineClientError> {
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
}
