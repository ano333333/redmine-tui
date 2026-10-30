use std::cell::RefCell;
use std::rc::Rc;

use super::download_attachment_original::{load_attachment_original, write_image};
use crate::clients::local_file::LocalFileClient;
use crate::clients::redmine::RedmineClient;
use crate::stores::{AttachmentImageAction, Dispatcher, ImageDisplay, ImageKey, ImageState};
use crate::usecases::{UsecaseClients, UsecaseOutput, UsecaseRequest, UsecaseTask};
use crate::vos::AttachmentId;

/// 取得するサムネイルの大きさ。Redmineが返せる最大の800pxにする。
pub(crate) const THUMBNAIL_SIZE: u32 = 800;
/// サムネイルとして受け取るバイト数の上限。
const THUMBNAIL_MAX_BYTES: u64 = 4 * 1024 * 1024;

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
    use crate::test_support::run_usecases;

    use super::super::image_test_support::{
        ImageStubClient, StubTempFileClient, TEMP_DIR, image_enabled_dispatcher, offline,
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
            vec![("attachment-1-thumbnail-800".to_string(), vec![1])]
        );
        assert!(output.requests.is_empty());
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
            vec![("attachment-1-original".to_string(), vec![2])]
        );
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Loaded { key, .. })] if *key == ORIGINAL
        ));
    }

    #[tokio::test]
    async fn fallback_request_loads_the_original() {
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
            Some(ImageState::Preparing { path })
                if path == &Path::new(TEMP_DIR).join("attachment-1-original")
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
