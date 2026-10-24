use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{AttachmentAction, AttachmentUploadStage, Dispatcher, NoticeAction, NoticeId};
use crate::usecases::{UsecaseOutput, UsecaseRequest, UsecaseTask};
use crate::vos::{IssueId, UploadToken};

/// 送ったファイルを`filename`の名前でIssueに紐付ける。
///
/// 成功したら、確認の取得の後続要求を返す。失敗したら、追加状態を削除して通知する。
/// tokenはRedmineで1回しか使えないため、紐付けを再試行しない。
///
/// # Panics
///
/// 添付ファイルの追加がAttachingでない場合にpanicする。
pub fn put_attachment_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
    token: UploadToken,
    filename: String,
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
        Some(AttachmentUploadStage::Attaching),
        "attachment upload of issue {issue_id} is not attaching"
    );
    Some(Box::pin(async move {
        if let Err(error) = client.attach_upload(issue_id, &token, &filename).await {
            return vec![
                NoticeAction::Push {
                    id: NoticeId::new(),
                    message: format!("Issue #{issue_id}へのファイルの添付に失敗しました: {error}"),
                }
                .into(),
                AttachmentAction::FailUpload { issue_id }.into(),
            ]
            .into();
        }
        UsecaseOutput {
            actions: vec![AttachmentAction::AttachSucceeded { issue_id }.into()],
            requests: vec![UsecaseRequest::ConfirmAttachmentUpload { issue_id }],
        }
    }))
}

#[cfg(test)]
mod tests {
    use crate::stores::Action;
    use crate::test_support::{dispatch_loaded_issue, sample_issue_aggregate};

    use super::super::attachment_test_support::{AttachmentStubClient, offline};
    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);

    fn attaching_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        dispatch_loaded_issue(
            &mut dispatcher,
            sample_issue_aggregate(1, "subject", 1.into(), None, None, None, 0),
        );
        for action in [
            AttachmentAction::StartUpload {
                issue_id: ISSUE_ID,
                filename: "sample.png".to_string(),
            },
            AttachmentAction::UploadSucceeded { issue_id: ISSUE_ID },
        ] {
            dispatcher.dispatch(action);
            dispatcher.consume_action();
        }
        Rc::new(RefCell::new(dispatcher))
    }

    #[tokio::test]
    async fn attached_upload_requests_the_confirmation() {
        let client = Arc::new(AttachmentStubClient::new());

        let output = put_attachment_upload(
            attaching_dispatcher(),
            client.clone(),
            ISSUE_ID,
            UploadToken::new("7.token"),
            "renamed.png".to_string(),
        )
        .expect("attaching should start")
        .await;

        assert_eq!(
            *client.attaches.lock().unwrap(),
            vec![(
                ISSUE_ID,
                UploadToken::new("7.token"),
                "renamed.png".to_string()
            )]
        );
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Attachment(AttachmentAction::AttachSucceeded { issue_id })]
                if *issue_id == ISSUE_ID
        ));
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::ConfirmAttachmentUpload { issue_id: ISSUE_ID }]
        );
    }

    #[tokio::test]
    async fn failed_attach_ends_the_upload_without_a_confirmation() {
        let client = Arc::new(AttachmentStubClient {
            attach_result: Err(offline()),
            ..AttachmentStubClient::new()
        });

        let output = put_attachment_upload(
            attaching_dispatcher(),
            client,
            ISSUE_ID,
            UploadToken::new("7.token"),
            "sample.png".to_string(),
        )
        .expect("attaching should start")
        .await;

        assert!(matches!(
            output.actions.as_slice(),
            [
                Action::Notice(NoticeAction::Push { .. }),
                Action::Attachment(AttachmentAction::FailUpload { issue_id })
            ] if *issue_id == ISSUE_ID
        ));
        assert!(output.requests.is_empty());
    }

    #[test]
    #[should_panic(expected = "attachment upload of issue 1 is not attaching")]
    fn attaching_while_uploading_the_file_panics() {
        let mut dispatcher = Dispatcher::new();
        dispatch_loaded_issue(
            &mut dispatcher,
            sample_issue_aggregate(1, "subject", 1.into(), None, None, None, 0),
        );
        dispatcher.dispatch(AttachmentAction::StartUpload {
            issue_id: ISSUE_ID,
            filename: "sample.png".to_string(),
        });
        dispatcher.consume_action();

        put_attachment_upload(
            Rc::new(RefCell::new(dispatcher)),
            Arc::new(AttachmentStubClient::new()),
            ISSUE_ID,
            UploadToken::new("7.token"),
            "sample.png".to_string(),
        );
    }
}
