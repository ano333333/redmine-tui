use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{AttachmentAction, AttachmentUploadStage, Dispatcher, NoticeAction, NoticeId};
use crate::usecases::{UsecaseOutput, UsecaseTask};
use crate::vos::IssueId;

use super::attachment_image_requests;
use super::upload_issue::fetch_issue_for_upload;

/// 添付ファイルを紐付けたIssueを取得し直し、添付ファイルと追加を記録したJournalを取り込む。
///
/// 取得に失敗しても、添付ファイルはすでにIssueに付いている。追加をやり直すと二重に添付されるため、
/// そのことを通知して追加状態を削除する。
///
/// # Panics
///
/// 添付ファイルの追加がConfirmingでない場合にpanicする。
pub fn confirm_attachment_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let stage = dispatcher
        .borrow()
        .store()
        .try_get_attachment_upload(issue_id)
        .map(|upload| upload.stage);
    assert_eq!(
        stage,
        Some(AttachmentUploadStage::Confirming),
        "attachment upload of issue {issue_id} is not confirming"
    );
    Some(Box::pin(async move {
        let actions = match fetch_issue_for_upload(client.as_ref(), issue_id).await {
            Ok(confirmed) => {
                return UsecaseOutput {
                    requests: attachment_image_requests(&confirmed.aggregate),
                    actions: vec![
                        AttachmentAction::ConfirmSucceeded {
                            issue: confirmed.aggregate,
                            children: confirmed.children,
                        }
                        .into(),
                    ],
                };
            }
            Err(reason) => vec![
                NoticeAction::Push {
                    id: NoticeId::new(),
                    message: format!(
                        "Issue #{issue_id}にファイルを添付しましたが、確認の取得に失敗しました。もう一度追加すると二重に添付されます: {reason}"
                    ),
                }
                .into(),
                AttachmentAction::FailUpload { issue_id }.into(),
            ],
        };
        actions.into()
    }))
}

#[cfg(test)]
mod tests {
    use crate::clients::redmine::base::FetchedIssue;
    use crate::entities::IssueAggregate;
    use crate::stores::{Action, AttachmentAction, NoticeAction};
    use crate::test_support::{
        complete_usecase, dispatch_loaded_issue, sample_attachment, sample_issue_aggregate,
    };
    use crate::usecases::UsecaseRequest;
    use crate::vos::AttachmentId;

    use super::super::attachment_test_support::{AttachmentStubClient, offline};
    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);

    fn issue(id: u16) -> IssueAggregate {
        sample_issue_aggregate(id, "subject", 1.into(), None, None, None, 0)
    }

    fn confirming_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        dispatch_loaded_issue(&mut dispatcher, issue(1));
        for action in [
            AttachmentAction::StartUpload {
                issue_id: ISSUE_ID,
                filename: "sample.png".to_string(),
            },
            AttachmentAction::UploadSucceeded { issue_id: ISSUE_ID },
            AttachmentAction::AttachSucceeded { issue_id: ISSUE_ID },
        ] {
            dispatcher.dispatch(action);
            dispatcher.consume_action();
        }
        Rc::new(RefCell::new(dispatcher))
    }

    fn client_returning(
        result: Result<FetchedIssue, crate::clients::redmine::RedmineClientError>,
    ) -> Arc<AttachmentStubClient> {
        let client = AttachmentStubClient::new();
        client.get_results.lock().unwrap().push_back(result);
        Arc::new(client)
    }

    #[tokio::test]
    async fn fetched_issue_completes_the_upload() {
        let client = client_returning(Ok(FetchedIssue {
            aggregate: issue(1),
            children: vec![],
        }));

        let actions = complete_usecase(confirm_attachment_upload(
            confirming_dispatcher(),
            client,
            ISSUE_ID,
        ))
        .await;

        assert!(matches!(
            actions.as_slice(),
            [Action::Attachment(AttachmentAction::ConfirmSucceeded { issue, .. })]
                if issue.issue.id == ISSUE_ID
        ));
    }

    #[tokio::test]
    async fn fetched_issue_requests_images_including_the_added_attachment() {
        let mut confirmed = issue(1);
        confirmed.attachments = vec![sample_attachment(5, Some("image/png"), true)];
        let client = client_returning(Ok(FetchedIssue {
            aggregate: confirmed,
            children: vec![],
        }));

        let output = confirm_attachment_upload(confirming_dispatcher(), client, ISSUE_ID)
            .expect("confirmation should start")
            .await;

        assert_eq!(
            output.requests,
            vec![UsecaseRequest::DownloadAttachmentImage {
                attachment_id: AttachmentId::new(5),
                thumbnailable: true,
            }]
        );
    }

    #[tokio::test]
    async fn failed_fetch_ends_the_upload_with_a_notice() {
        let client = client_returning(Err(offline()));

        let actions = complete_usecase(confirm_attachment_upload(
            confirming_dispatcher(),
            client,
            ISSUE_ID,
        ))
        .await;

        let [
            Action::Notice(NoticeAction::Push { message, .. }),
            Action::Attachment(AttachmentAction::FailUpload { issue_id }),
        ] = actions.as_slice()
        else {
            panic!("expected a notice and FailUpload");
        };
        assert!(message.contains("二重に添付されます"));
        assert_eq!(*issue_id, ISSUE_ID);
    }

    #[tokio::test]
    async fn fetch_of_another_issue_is_treated_as_a_failure() {
        let client = client_returning(Ok(FetchedIssue {
            aggregate: issue(2),
            children: vec![],
        }));

        let actions = complete_usecase(confirm_attachment_upload(
            confirming_dispatcher(),
            client,
            ISSUE_ID,
        ))
        .await;

        assert!(matches!(
            actions.as_slice(),
            [
                Action::Notice(_),
                Action::Attachment(AttachmentAction::FailUpload { .. })
            ]
        ));
    }

    #[test]
    #[should_panic(expected = "attachment upload of issue 1 is not confirming")]
    fn confirming_before_attaching_panics() {
        let mut dispatcher = Dispatcher::new();
        dispatch_loaded_issue(&mut dispatcher, issue(1));

        confirm_attachment_upload(
            Rc::new(RefCell::new(dispatcher)),
            Arc::new(AttachmentStubClient::new()),
            ISSUE_ID,
        );
    }
}
