use std::cell::RefCell;
use std::rc::Rc;

use super::download_attachment_original::{load_attachment_original, write_image};
use crate::clients::local_file::LocalFileClient;
use crate::clients::redmine::RedmineClient;
use crate::entities::IssueAggregate;
use crate::stores::{AttachmentImageAction, Dispatcher, ImageDisplay, ImageKey, ImageState};
use crate::usecases::image::IMAGE_MAX_SIDE;
use crate::usecases::{UsecaseClients, UsecaseOutput, UsecaseRequest, UsecaseTask};
use crate::vos::AttachmentId;

/// 取得するサムネイルの大きさ。前処理で縮小しなくて済むよう、前処理後の長辺の上限に合わせる。
const THUMBNAIL_SIZE: u32 = IMAGE_MAX_SIDE;
/// サムネイルとして受け取るバイト数の上限。
const THUMBNAIL_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// SVGは`image` crateでdecodeできないため、画像の添付ファイルとして扱わない。
const SVG_CONTENT_TYPE: &str = "image/svg+xml";

/// Issue詳細の取得結果に含まれる画像の添付ファイルについて、表示に使う画像の取得要求を返す。
///
/// 完了したtaskからはStoreを参照できないため、取得済みかどうかでは絞らない。
/// 取得済みの画像は、[`download_attachment_image`]が起動時に除く。
pub(super) fn attachment_image_requests(issue: &IssueAggregate) -> Vec<UsecaseRequest> {
    issue
        .attachments
        .iter()
        .filter(|attachment| {
            attachment
                .content_type
                .as_deref()
                .is_some_and(|content_type| {
                    content_type.starts_with("image/") && content_type != SVG_CONTENT_TYPE
                })
        })
        .map(|attachment| UsecaseRequest::DownloadAttachmentImage {
            attachment_id: attachment.id,
            thumbnailable: attachment.thumbnailable,
        })
        .collect()
}

/// 添付ファイルの表示に使う画像を取得し、一時ファイルへ書き込む。
///
/// `thumbnailable`ならサムネイルを、そうでなければ原画像を取得する。サムネイルを取得できなければ、
/// 原画像の取得へ切り替えて、その後続要求を返す。
///
/// 画像表示が無効な場合と、添付ファイルの画像を読み込み中か読み込み済みの場合は起動しない。
pub fn download_attachment_image<C, F>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    clients: UsecaseClients<C, F>,
    attachment_id: AttachmentId,
    thumbnailable: bool,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
    F: LocalFileClient + Send + Sync + 'static,
{
    let can_start = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        matches!(store.get_image_display(), ImageDisplay::Enabled { .. })
            && store
                .try_get_attachment_image_key(attachment_id)
                .is_none_or(|key| {
                    matches!(store.try_get_image(key), Some(ImageState::Failed { .. }))
                })
    };
    if !can_start {
        return None;
    }

    let key = if thumbnailable {
        ImageKey::AttachmentThumbnail {
            id: attachment_id,
            size: THUMBNAIL_SIZE,
        }
    } else {
        ImageKey::AttachmentOriginal { id: attachment_id }
    };
    dispatcher
        .borrow_mut()
        .dispatch(AttachmentImageAction::Start { attachment_id, key });
    if !thumbnailable {
        return Some(Box::pin(load_attachment_original(clients, attachment_id)));
    }
    Some(Box::pin(async move {
        match clients
            .redmine
            .get_attachment_thumbnail(attachment_id, THUMBNAIL_SIZE, THUMBNAIL_MAX_BYTES)
            .await
        {
            Ok(content) => write_image(clients.local_file.as_ref(), key, content).await,
            Err(error) => UsecaseOutput {
                actions: vec![
                    AttachmentImageAction::FallBackToOriginal {
                        attachment_id,
                        reason: error.to_string(),
                    }
                    .into(),
                ],
                requests: vec![UsecaseRequest::DownloadAttachmentOriginal { attachment_id }],
            },
        }
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use crate::stores::{Action, ImageAction};
    use crate::test_support::{run_usecases, sample_attachment, sample_issue_aggregate};

    use super::super::image_test_support::{
        ImageStubClient, StubTempFileClient, TEMP_DIR, image_enabled_dispatcher, offline,
        original_png, thumbnail_png,
    };
    use super::*;

    const ATTACHMENT_ID: AttachmentId = AttachmentId::new(1);
    const THUMBNAIL: ImageKey = ImageKey::AttachmentThumbnail {
        id: ATTACHMENT_ID,
        size: 800,
    };
    const ORIGINAL: ImageKey = ImageKey::AttachmentOriginal { id: ATTACHMENT_ID };

    fn clients(
        redmine: ImageStubClient,
        local_file: StubTempFileClient,
    ) -> UsecaseClients<ImageStubClient, StubTempFileClient> {
        UsecaseClients {
            redmine: Arc::new(redmine),
            local_file: Arc::new(local_file),
        }
    }

    fn start(
        dispatcher: &Rc<RefCell<Dispatcher>>,
        clients: &UsecaseClients<ImageStubClient, StubTempFileClient>,
        thumbnailable: bool,
    ) -> Option<UsecaseTask> {
        let task = download_attachment_image(
            dispatcher.clone(),
            clients.clone(),
            ATTACHMENT_ID,
            thumbnailable,
        );
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        task
    }

    fn apply(dispatcher: &Rc<RefCell<Dispatcher>>, output: UsecaseOutput) {
        for action in output.actions {
            dispatcher.borrow_mut().dispatch(action);
            dispatcher.borrow_mut().consume_action();
        }
    }

    #[test]
    fn only_image_attachments_are_requested() {
        let mut issue = sample_issue_aggregate(1, "subject", 1.into(), None, None, None, 0);
        issue.attachments = vec![
            sample_attachment(1, Some("image/png"), true),
            sample_attachment(2, Some("image/svg+xml"), false),
            sample_attachment(3, Some("application/pdf"), true),
            sample_attachment(4, None, false),
        ];

        assert_eq!(
            attachment_image_requests(&issue),
            vec![UsecaseRequest::DownloadAttachmentImage {
                attachment_id: AttachmentId::new(1),
                thumbnailable: true,
            }]
        );
    }

    #[tokio::test]
    async fn thumbnail_is_written_to_a_temporary_file() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let clients = clients(ImageStubClient::new(), StubTempFileClient::new());

        let task = start(&dispatcher, &clients, true).expect("download should start");
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .try_get_attachment_image_key(ATTACHMENT_ID),
            Some(THUMBNAIL)
        );
        let output = task.await;

        assert_eq!(
            *clients.redmine.thumbnail_gets.lock().unwrap(),
            vec![(ATTACHMENT_ID, 800, 4_194_304)]
        );
        assert_eq!(
            *clients.local_file.writes.lock().unwrap(),
            vec![("attachment-1-thumbnail-800".to_string(), thumbnail_png())]
        );
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::PrepareImage {
                key: THUMBNAIL,
                path: Path::new(TEMP_DIR).join("attachment-1-thumbnail-800"),
            }]
        );
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Loaded { key, path })]
                if *key == THUMBNAIL && path == &Path::new(TEMP_DIR).join("attachment-1-thumbnail-800")
        ));
    }

    #[tokio::test]
    async fn failed_thumbnail_falls_back_to_the_original() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let mut redmine = ImageStubClient::new();
        redmine.thumbnail_result = Err(offline());
        let clients = clients(redmine, StubTempFileClient::new());

        let output = start(&dispatcher, &clients, true)
            .expect("download should start")
            .await;

        assert!(clients.local_file.writes.lock().unwrap().is_empty());
        assert!(matches!(
            output.actions.as_slice(),
            [Action::AttachmentImage(AttachmentImageAction::FallBackToOriginal { attachment_id, reason })]
                if *attachment_id == ATTACHMENT_ID && reason == "network error: offline"
        ));
        assert_eq!(
            output.requests,
            vec![UsecaseRequest::DownloadAttachmentOriginal {
                attachment_id: ATTACHMENT_ID
            }]
        );
    }

    #[tokio::test]
    async fn failed_write_fails_the_thumbnail_without_falling_back() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let mut local_file = StubTempFileClient::new();
        local_file.fails = true;
        let clients = clients(ImageStubClient::new(), local_file);

        let output = start(&dispatcher, &clients, true)
            .expect("download should start")
            .await;

        assert!(output.requests.is_empty());
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Fail { key, .. })] if *key == THUMBNAIL
        ));
    }

    #[tokio::test]
    async fn attachment_without_thumbnail_downloads_the_original() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let clients = clients(ImageStubClient::new(), StubTempFileClient::new());

        let output = start(&dispatcher, &clients, false)
            .expect("download should start")
            .await;

        assert!(clients.redmine.thumbnail_gets.lock().unwrap().is_empty());
        assert_eq!(
            *clients.local_file.writes.lock().unwrap(),
            vec![("attachment-1-original".to_string(), original_png())]
        );
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Loaded { key, .. })] if *key == ORIGINAL
        ));
    }

    #[tokio::test]
    async fn fallback_request_prepares_the_original() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let mut redmine = ImageStubClient::new();
        redmine.thumbnail_result = Err(offline());
        let clients = clients(redmine, StubTempFileClient::new());

        run_usecases(
            UsecaseRequest::DownloadAttachmentImage {
                attachment_id: ATTACHMENT_ID,
                thumbnailable: true,
            },
            dispatcher.clone(),
            clients,
        )
        .await;

        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        assert_eq!(
            store.try_get_attachment_image_key(ATTACHMENT_ID),
            Some(ORIGINAL)
        );
        assert!(matches!(
            store.try_get_image(ORIGINAL),
            Some(ImageState::Ready(image)) if image.pixel_size() == (16, 16)
        ));
    }

    #[test]
    fn disabled_image_display_does_not_start() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let clients = clients(ImageStubClient::new(), StubTempFileClient::new());

        assert!(start(&dispatcher, &clients, true).is_none());
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .try_get_attachment_image_key(ATTACHMENT_ID),
            None
        );
    }

    #[test]
    fn image_in_progress_is_not_downloaded_again() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let clients = clients(ImageStubClient::new(), StubTempFileClient::new());
        let _task = start(&dispatcher, &clients, true).expect("download should start");

        assert!(start(&dispatcher, &clients, true).is_none());
    }

    #[tokio::test]
    async fn failed_image_is_downloaded_again() {
        let dispatcher = Rc::new(RefCell::new(image_enabled_dispatcher()));
        let mut local_file = StubTempFileClient::new();
        local_file.fails = true;
        let clients = clients(ImageStubClient::new(), local_file);
        let output = start(&dispatcher, &clients, true)
            .expect("download should start")
            .await;
        apply(&dispatcher, output);

        assert!(start(&dispatcher, &clients, true).is_some());
    }
}
