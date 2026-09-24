use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::entities::IssueAggregate;
use crate::stores::{Action, Dispatcher, IssueAction, IssueState, NoticeAction, NoticeId};
use crate::vos::{IssueId, IssuePropertyDiff};

use super::{apply_issue_property_diffs, fetch_issue_with_conflicts};

/// 競合がないことを確認済みの Issue を Redmine サーバーへアップロードする。
pub(crate) async fn upload_issue(
    client: &impl RedmineClient,
    issue: &IssueAggregate,
) -> Result<(), RedmineClientError> {
    client.update_issue(issue).await
}

/// 編集済みのIssueのuploadを開始する。
///
/// 呼び出し時に状態を検証し、`StartUpload`を同期的にqueueへ追加してproperty diffをsnapshotする。
/// 返却したFutureは競合確認GETとIssue PUTを行い、完了Actionを返す。
///
/// # Panics
///
/// Issueが未登録、またはEdited以外の場合にpanicする。
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
        let (_, state) = dispatcher
            .store()
            .get_issue(id)
            .expect("tried to upload unknown issue");
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
    let (mut server_issue, conflicts) = match fetch_issue_with_conflicts(client, id, diffs).await {
        Ok(result) => result,
        Err(error) => {
            return issue_upload_failure_actions(id, error.to_string());
        }
    };
    if !conflicts.is_empty() {
        return vec![
            IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
            }
            .into(),
        ];
    }

    apply_issue_property_diffs(&mut server_issue, diffs);
    if let Err(error) = upload_issue(client, &server_issue).await {
        return issue_upload_failure_actions(id, error.to_string());
    }

    vec![
        IssueAction::Sync {
            issue: server_issue,
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
            created_at: chrono::Local::now(),
        }
        .into(),
        IssueAction::FailUpload { id, message }.into(),
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::clients::redmine::base::FetchedIssue;
    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, IssueAggregate, IssueStatus, Priority, Project, TargetVersion,
        TimeEntityActivity, Tracker, User,
    };
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId};

    use super::upload_issue;

    #[tokio::test]
    async fn uploads_the_supplied_issue() {
        let client = StubClient::succeeds();
        let issue = sample_issue_aggregate(
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
        assert_eq!(uploaded[0].issue.id, issue.issue.id);
        assert_eq!(uploaded[0].issue.subject, issue.issue.subject);
    }

    #[tokio::test]
    async fn propagates_client_error() {
        let expected = RedmineClientError::Network {
            reason: "offline".to_string(),
        };
        let client = StubClient::fails(expected.clone());
        let issue =
            sample_issue_aggregate(7, "subject", IssueStatusId::new(1), None, None, None, 0);

        let actual = upload_issue(&client, &issue).await.unwrap_err();

        assert_eq!(actual, expected);
    }

    struct StubClient {
        uploaded: Mutex<Vec<IssueAggregate>>,
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
        async fn update_issue(&self, issue: &IssueAggregate) -> Result<(), RedmineClientError> {
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            self.uploaded.lock().unwrap().push(issue.clone());
            Ok(())
        }

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_issue_notes(
            &self,
            _: crate::vos::IssueId,
            _: &str,
        ) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
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
