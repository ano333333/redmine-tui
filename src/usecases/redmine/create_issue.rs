use crate::clients::local_file::LocalFileClient;
use crate::clients::redmine::base::NewIssueUpload;
use crate::clients::redmine::{NewIssue, RedmineClient, RedmineClientError};
use crate::usecases::{
    IssueCreateFailure, IssueCreationId, NewIssueDraft, UsecaseClients, UsecaseEvent,
    UsecaseFailure, UsecaseOutput, UsecaseRequest, UsecaseTask,
};

use super::upload_local_file::{UploadLocalFileError, upload_local_file};

/// 成功したら作成完了のイベントと、作成したIssueの取得を後続要求として返す。いずれかの段階で
/// 失敗したら、作成の失敗を通知する。
pub fn create_issue<C, F>(
    clients: UsecaseClients<C, F>,
    creation_id: IssueCreationId,
    draft: NewIssueDraft,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
    F: LocalFileClient + Send + Sync + 'static,
{
    Some(Box::pin(async move {
        let failure = |cause: IssueCreateFailure| UsecaseOutput {
            actions: Vec::new(),
            requests: Vec::new(),
            events: vec![UsecaseEvent::Failure(UsecaseFailure::IssueCreate {
                creation_id,
                cause,
            })],
        };
        let mut uploads = Vec::with_capacity(draft.attachments.len());
        for attachment in &draft.attachments {
            let filename = attachment.filename.clone();
            match upload_local_file(&clients, &filename, &attachment.path).await {
                Ok(token) => uploads.push(NewIssueUpload { token, filename }),
                Err(UploadLocalFileError::Read(error)) => {
                    return failure(IssueCreateFailure::AttachmentFileRead {
                        filename,
                        reason: error.to_string(),
                    });
                }
                Err(UploadLocalFileError::Upload(error)) => {
                    return failure(IssueCreateFailure::AttachmentFileUpload {
                        filename,
                        reason: error.to_string(),
                    });
                }
            }
        }
        match clients
            .redmine
            .create_issue(&new_issue(draft, uploads))
            .await
        {
            Ok(id) => UsecaseOutput {
                actions: Vec::new(),
                requests: vec![UsecaseRequest::FetchIssue {
                    id,
                    with_parent: true,
                }],
                events: vec![UsecaseEvent::IssueCreated { creation_id, id }],
            },
            // 4xxは、Redmineが作成しなかったことを表す。5xxは前段のproxyが返すこともあり、
            // 通信やClientのエラーとともに、Redmineが作成した後に応答を受け取れなかった場合を含む。
            Err(
                error @ (RedmineClientError::Network { .. }
                | RedmineClientError::Client { .. }
                | RedmineClientError::InternalServerError { .. }),
            ) => failure(IssueCreateFailure::CreateOutcomeUnknown {
                reason: error.to_string(),
            }),
            Err(error) => failure(IssueCreateFailure::Create {
                reason: error.to_string(),
            }),
        }
    }))
}

fn new_issue(draft: NewIssueDraft, uploads: Vec<NewIssueUpload>) -> NewIssue {
    NewIssue {
        project_id: draft.project_id,
        subject: draft.subject,
        parent_issue_id: draft.parent_issue_id,
        description: Some(draft.description).filter(|description| !description.is_empty()),
        tracker_id: draft.tracker_id,
        status_id: draft.status_id,
        priority_id: draft.priority_id,
        assigned_to_id: draft.assigned_to_id,
        target_version_id: draft.target_version_id,
        start_date: draft.start_date,
        due_date: draft.due_date,
        done_ratio: draft.done_ratio,
        estimated_hours: draft.estimated_hours,
        category_id: draft.category_id,
        uploads,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use crate::clients::local_file::LocalFileError;
    use crate::clients::redmine::RedmineClientError;
    use crate::test_support::RedmineClientStub;
    use crate::usecases::DraftAttachment;
    use crate::vos::{IssueId, ProjectId, TrackerId, UploadToken};

    use super::super::attachment_test_support::{StubLocalFileClient, offline};
    use super::*;

    struct CreateStubClient {
        upload_result: Result<UploadToken, RedmineClientError>,
        create_result: Result<IssueId, RedmineClientError>,
        uploads: Mutex<Vec<(String, Vec<u8>)>>,
        creates: Mutex<Vec<NewIssue>>,
    }

    impl CreateStubClient {
        fn new() -> Self {
            Self {
                upload_result: Ok(UploadToken::new("7.token")),
                create_result: Ok(IssueId::new(99)),
                uploads: Mutex::new(Vec::new()),
                creates: Mutex::new(Vec::new()),
            }
        }
    }

    impl RedmineClientStub for CreateStubClient {
        async fn upload_file(
            &self,
            filename: &str,
            content: Vec<u8>,
        ) -> Result<UploadToken, RedmineClientError> {
            self.uploads
                .lock()
                .unwrap()
                .push((filename.to_string(), content));
            self.upload_result.clone()
        }

        async fn create_issue(&self, issue: &NewIssue) -> Result<IssueId, RedmineClientError> {
            self.creates.lock().unwrap().push(issue.clone());
            self.create_result.clone()
        }
    }

    fn clients(
        redmine: CreateStubClient,
        local_file: StubLocalFileClient,
    ) -> UsecaseClients<CreateStubClient, StubLocalFileClient> {
        UsecaseClients {
            redmine: Arc::new(redmine),
            local_file: Arc::new(local_file),
        }
    }

    fn draft() -> NewIssueDraft {
        NewIssueDraft {
            subject: "新しいIssue".to_string(),
            tracker_id: Some(TrackerId::new(2)),
            ..NewIssueDraft::new(ProjectId::new(1), Some(IssueId::new(3)))
        }
    }

    fn draft_with_attachment() -> NewIssueDraft {
        NewIssueDraft {
            attachments: vec![DraftAttachment {
                filename: "screen.png".to_string(),
                path: PathBuf::from("/tmp/sample.png"),
            }],
            ..draft()
        }
    }

    fn assert_failure(
        output: &UsecaseOutput,
        creation_id: IssueCreationId,
        cause: IssueCreateFailure,
    ) {
        assert!(output.actions.is_empty());
        assert!(output.requests.is_empty());
        assert_eq!(
            output.events,
            vec![UsecaseEvent::Failure(UsecaseFailure::IssueCreate {
                creation_id,
                cause,
            })]
        );
    }

    #[tokio::test]
    async fn created_issue_is_announced_and_fetched_without_changing_the_store() {
        let clients = clients(
            CreateStubClient::new(),
            StubLocalFileClient::new(Ok(vec![])),
        );
        let creation_id = IssueCreationId::new();

        let output = create_issue(clients.clone(), creation_id, draft())
            .expect("creation always runs asynchronously")
            .await;

        assert!(output.actions.is_empty());
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::FetchIssue {
                id: IssueId::new(99),
                with_parent: true,
            }]
        );
        assert_eq!(
            output.events,
            vec![UsecaseEvent::IssueCreated {
                creation_id,
                id: IssueId::new(99),
            }]
        );
        let creates = clients.redmine.creates.lock().unwrap();
        let [created] = creates.as_slice() else {
            panic!("expected one creation, got {creates:?}");
        };
        assert_eq!(created.project_id, ProjectId::new(1));
        assert_eq!(created.parent_issue_id, Some(IssueId::new(3)));
        assert_eq!(created.subject, "新しいIssue");
        assert_eq!(created.tracker_id, Some(TrackerId::new(2)));
        assert_eq!(created.description, None);
        assert!(created.uploads.is_empty());
    }

    #[tokio::test]
    async fn attachments_are_read_and_uploaded_before_creating() {
        let clients = clients(
            CreateStubClient::new(),
            StubLocalFileClient::new(Ok(vec![1, 2, 3])),
        );

        create_issue(
            clients.clone(),
            IssueCreationId::new(),
            draft_with_attachment(),
        )
        .expect("creation always runs asynchronously")
        .await;

        assert_eq!(
            *clients.local_file.reads.lock().unwrap(),
            vec![(Path::new("/tmp/sample.png").to_path_buf(), 5_242_880)]
        );
        assert_eq!(
            *clients.redmine.uploads.lock().unwrap(),
            vec![("screen.png".to_string(), vec![1, 2, 3])]
        );
        assert_eq!(
            clients.redmine.creates.lock().unwrap()[0].uploads,
            vec![NewIssueUpload {
                token: UploadToken::new("7.token"),
                filename: "screen.png".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn unreadable_attachment_fails_without_sending() {
        let clients = clients(
            CreateStubClient::new(),
            StubLocalFileClient::new(Err(LocalFileError {
                path: PathBuf::from("/tmp/sample.png"),
                reason: "not found".to_string(),
            })),
        );
        let creation_id = IssueCreationId::new();

        let output = create_issue(clients.clone(), creation_id, draft_with_attachment())
            .expect("creation always runs asynchronously")
            .await;

        assert_failure(
            &output,
            creation_id,
            IssueCreateFailure::AttachmentFileRead {
                filename: "screen.png".to_string(),
                reason: "failed to read /tmp/sample.png: not found".to_string(),
            },
        );
        assert!(clients.redmine.uploads.lock().unwrap().is_empty());
        assert!(clients.redmine.creates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn failed_upload_fails_without_creating() {
        let clients = clients(
            CreateStubClient {
                upload_result: Err(offline()),
                ..CreateStubClient::new()
            },
            StubLocalFileClient::new(Ok(vec![1])),
        );
        let creation_id = IssueCreationId::new();

        let output = create_issue(clients.clone(), creation_id, draft_with_attachment())
            .expect("creation always runs asynchronously")
            .await;

        assert_failure(
            &output,
            creation_id,
            IssueCreateFailure::AttachmentFileUpload {
                filename: "screen.png".to_string(),
                reason: "network error: offline".to_string(),
            },
        );
        assert!(clients.redmine.creates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejected_creation_is_reported_as_a_failure() {
        let clients = clients(
            CreateStubClient {
                create_result: Err(RedmineClientError::UnprocessableEntity {
                    context: crate::clients::redmine::RedmineHttpError {
                        method: "POST".to_string(),
                        url: "/issues.json".to_string(),
                        status_code: 422,
                        response_body: r#"{"errors":["Subject cannot be blank"]}"#.to_string(),
                    },
                }),
                ..CreateStubClient::new()
            },
            StubLocalFileClient::new(Ok(vec![])),
        );
        let creation_id = IssueCreationId::new();

        let output = create_issue(clients, creation_id, draft())
            .expect("creation always runs asynchronously")
            .await;

        assert_failure(
            &output,
            creation_id,
            IssueCreateFailure::Create {
                reason: r#"unprocessable entity: POST /issues.json returned 422 with body: {"errors":["Subject cannot be blank"]}"#
                    .to_string(),
            },
        );
    }

    #[tokio::test]
    async fn gateway_timeout_on_creation_is_reported_with_an_unknown_outcome() {
        let clients = clients(
            CreateStubClient {
                create_result: Err(RedmineClientError::InternalServerError {
                    context: crate::clients::redmine::RedmineHttpError {
                        method: "POST".to_string(),
                        url: "/issues.json".to_string(),
                        status_code: 504,
                        response_body: "Gateway Timeout".to_string(),
                    },
                }),
                ..CreateStubClient::new()
            },
            StubLocalFileClient::new(Ok(vec![])),
        );
        let creation_id = IssueCreationId::new();

        let output = create_issue(clients, creation_id, draft())
            .expect("creation always runs asynchronously")
            .await;

        assert_failure(
            &output,
            creation_id,
            IssueCreateFailure::CreateOutcomeUnknown {
                reason: "internal server error: POST /issues.json returned 504 with body: Gateway Timeout"
                    .to_string(),
            },
        );
    }

    #[tokio::test]
    async fn creation_interrupted_by_the_network_is_reported_with_an_unknown_outcome() {
        let clients = clients(
            CreateStubClient {
                create_result: Err(offline()),
                ..CreateStubClient::new()
            },
            StubLocalFileClient::new(Ok(vec![])),
        );
        let creation_id = IssueCreationId::new();

        let output = create_issue(clients, creation_id, draft())
            .expect("creation always runs asynchronously")
            .await;

        assert_failure(
            &output,
            creation_id,
            IssueCreateFailure::CreateOutcomeUnknown {
                reason: "network error: offline".to_string(),
            },
        );
    }
}
