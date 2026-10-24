use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::clients::local_file::LocalFileClient;
use crate::clients::redmine::RedmineClient;
use crate::stores::{AttachmentAction, Dispatcher, IssueState, NoticeAction, NoticeId};
use crate::usecases::{UsecaseClients, UsecaseOutput, UsecaseRequest, UsecaseTask};
use crate::vos::IssueId;

/// Redmineの添付ファイルサイズ上限の設定`attachment_max_size`の既定値（5120 KB）。
///
/// RedmineのREST APIは設定値を返さないため、既定値に合わせる。サーバーが上限を下げている場合は
/// Redmineが送信を422で拒否し、上げている場合はこの値を超えるファイルを送れない。
const ATTACHMENT_MAX_BYTES: u64 = 5120 * 1024;

/// `path`のファイルを`filename`の名前でIssueへ添付する処理を始める。
///
/// `StartUpload`を同期的にqueueへ追加する。返却したtaskはファイルを読み込んでRedmineへ送り、
/// 成功したらtokenを載せた紐付けの後続要求を返す。読み込みか送信に失敗したら、追加状態を
/// 削除して通知する。
///
/// # Panics
///
/// Issueが取得済みのSyncedまたはEditedでない場合、またはIssueのJournalか別の添付ファイルを
/// 保存中の場合にpanicする。
pub fn start_attachment_upload<C, F>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    clients: UsecaseClients<C, F>,
    issue_id: IssueId,
    filename: String,
    path: PathBuf,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
    F: LocalFileClient + Send + Sync + 'static,
{
    {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        assert!(
            matches!(
                store.try_get_issue_state(issue_id),
                Some(IssueState::Synced | IssueState::Edited)
            ) && !store.has_uploading_journal(issue_id)
                && store.try_get_attachment_upload(issue_id).is_none(),
            "cannot start attachment upload of issue {issue_id}"
        );
    }
    dispatcher
        .borrow_mut()
        .dispatch(AttachmentAction::StartUpload {
            issue_id,
            filename: filename.clone(),
        });
    Some(Box::pin(async move {
        let content = match clients
            .local_file
            .read_file(&path, ATTACHMENT_MAX_BYTES)
            .await
        {
            Ok(content) => content,
            Err(error) => {
                return failure_output(
                    issue_id,
                    format!("Issue #{issue_id}へ添付するファイルを読み込めませんでした: {error}"),
                );
            }
        };
        let token = match clients.redmine.upload_file(&filename, content).await {
            Ok(token) => token,
            Err(error) => {
                return failure_output(
                    issue_id,
                    format!("Issue #{issue_id}へ添付するファイルの送信に失敗しました: {error}"),
                );
            }
        };
        UsecaseOutput {
            actions: vec![AttachmentAction::UploadSucceeded { issue_id }.into()],
            requests: vec![UsecaseRequest::PutAttachmentUpload {
                issue_id,
                token,
                filename,
            }],
        }
    }))
}

fn failure_output(issue_id: IssueId, message: String) -> UsecaseOutput {
    vec![
        NoticeAction::Push {
            id: NoticeId::new(),
            message,
        }
        .into(),
        AttachmentAction::FailUpload { issue_id }.into(),
    ]
    .into()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use crate::clients::local_file::LocalFileError;
    use crate::stores::{Action, AttachmentUploadStage, JournalAction};
    use crate::test_support::{dispatch_loaded_issue, sample_issue_aggregate};
    use crate::vos::UploadToken;

    use super::super::attachment_test_support::{
        AttachmentStubClient, StubLocalFileClient, offline,
    };
    use super::*;

    const ISSUE_ID: IssueId = IssueId::new(1);

    fn loaded_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        dispatch_loaded_issue(
            &mut dispatcher,
            sample_issue_aggregate(1, "subject", 1.into(), None, None, None, 0),
        );
        Rc::new(RefCell::new(dispatcher))
    }

    fn clients(
        redmine: AttachmentStubClient,
        local_file: StubLocalFileClient,
    ) -> UsecaseClients<AttachmentStubClient, StubLocalFileClient> {
        UsecaseClients {
            redmine: Arc::new(redmine),
            local_file: Arc::new(local_file),
        }
    }

    fn start(
        dispatcher: &Rc<RefCell<Dispatcher>>,
        clients: &UsecaseClients<AttachmentStubClient, StubLocalFileClient>,
    ) -> UsecaseTask {
        let task = start_attachment_upload(
            dispatcher.clone(),
            clients.clone(),
            ISSUE_ID,
            "renamed.png".to_string(),
            PathBuf::from("/tmp/sample.png"),
        )
        .expect("attachment upload should start");
        dispatcher.borrow_mut().consume_action();
        task
    }

    #[tokio::test]
    async fn read_file_is_uploaded_and_the_attach_is_requested() {
        let dispatcher = loaded_dispatcher();
        let clients = clients(
            AttachmentStubClient::new(),
            StubLocalFileClient::new(Ok(vec![1, 2, 3])),
        );

        let task = start(&dispatcher, &clients);
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .try_get_attachment_upload(ISSUE_ID)
                .map(|upload| upload.stage),
            Some(AttachmentUploadStage::Uploading)
        );
        let output = task.await;

        assert_eq!(
            *clients.local_file.reads.lock().unwrap(),
            vec![(Path::new("/tmp/sample.png").to_path_buf(), 5_242_880)]
        );
        assert_eq!(
            *clients.redmine.uploads.lock().unwrap(),
            vec![("renamed.png".to_string(), vec![1, 2, 3])]
        );
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Attachment(AttachmentAction::UploadSucceeded { issue_id })]
                if *issue_id == ISSUE_ID
        ));
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::PutAttachmentUpload {
                issue_id: ISSUE_ID,
                token: UploadToken::new("7.token"),
                filename: "renamed.png".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn unreadable_file_ends_the_upload_without_sending() {
        let dispatcher = loaded_dispatcher();
        let clients = clients(
            AttachmentStubClient::new(),
            StubLocalFileClient::new(Err(LocalFileError {
                path: PathBuf::from("/tmp/sample.png"),
                reason: "not found".to_string(),
            })),
        );

        let output = start(&dispatcher, &clients).await;

        assert!(clients.redmine.uploads.lock().unwrap().is_empty());
        assert!(matches!(
            output.actions.as_slice(),
            [
                Action::Notice(NoticeAction::Push { .. }),
                Action::Attachment(AttachmentAction::FailUpload { issue_id })
            ] if *issue_id == ISSUE_ID
        ));
        assert!(output.requests.is_empty());
    }

    #[tokio::test]
    async fn failed_upload_ends_the_upload_without_attaching() {
        let dispatcher = loaded_dispatcher();
        let clients = clients(
            AttachmentStubClient {
                upload_result: Err(offline()),
                ..AttachmentStubClient::new()
            },
            StubLocalFileClient::new(Ok(vec![1])),
        );

        let output = start(&dispatcher, &clients).await;

        assert!(matches!(
            output.actions.as_slice(),
            [
                Action::Notice(NoticeAction::Push { .. }),
                Action::Attachment(AttachmentAction::FailUpload { .. })
            ]
        ));
        assert!(output.requests.is_empty());
    }

    #[test]
    #[should_panic(expected = "cannot start attachment upload of issue 1")]
    fn starting_while_a_journal_is_uploading_panics() {
        let dispatcher = loaded_dispatcher();
        for action in [
            JournalAction::CreateLocal { issue_id: ISSUE_ID },
            JournalAction::StartLocalUpload { issue_id: ISSUE_ID },
        ] {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }

        start_attachment_upload(
            dispatcher,
            clients(
                AttachmentStubClient::new(),
                StubLocalFileClient::new(Ok(vec![])),
            ),
            ISSUE_ID,
            "sample.png".to_string(),
            PathBuf::from("/tmp/sample.png"),
        );
    }
}
