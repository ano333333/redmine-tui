use std::cell::RefCell;
use std::rc::Rc;

use crate::clients::local_file::LocalFileClient;
use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, ImageAction, ImageKey, ImageState};
use crate::usecases::{UsecaseClients, UsecaseOutput, UsecaseTask};
use crate::vos::AttachmentId;

/// 原画像として受け取るバイト数の上限。原画像は大きさに上限がないため、前処理の時間とメモリを抑える。
const ORIGINAL_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// サムネイルを取得できなかった添付ファイルの原画像を取得し、一時ファイルへ書き込む。
///
/// # Panics
///
/// 添付ファイルの表示に使う画像が、読み込み中の原画像でない場合にpanicする。
pub fn download_attachment_original<C, F>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    clients: UsecaseClients<C, F>,
    attachment_id: AttachmentId,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
    F: LocalFileClient + Send + Sync + 'static,
{
    {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        let key = store.try_get_attachment_image_key(attachment_id);
        assert!(
            key == Some(ImageKey::AttachmentOriginal { id: attachment_id })
                && matches!(
                    key.and_then(|key| store.try_get_image(key)),
                    Some(ImageState::Loading)
                ),
            "cannot download the original image of attachment {attachment_id} while it uses {key:?}"
        );
    }
    Some(Box::pin(load_attachment_original(clients, attachment_id)))
}

/// 原画像の取得を始めた添付ファイルについて、原画像を取得して一時ファイルへ書き込む。
pub(super) async fn load_attachment_original<C, F>(
    clients: UsecaseClients<C, F>,
    attachment_id: AttachmentId,
) -> UsecaseOutput
where
    C: RedmineClient + Send + Sync + 'static,
    F: LocalFileClient + Send + Sync + 'static,
{
    let key = ImageKey::AttachmentOriginal { id: attachment_id };
    match clients
        .redmine
        .get_attachment_content(attachment_id, ORIGINAL_MAX_BYTES)
        .await
    {
        Ok(content) => write_image(clients.local_file.as_ref(), key, content).await,
        Err(error) => vec![
            ImageAction::Fail {
                key,
                reason: error.to_string(),
            }
            .into(),
        ]
        .into(),
    }
}

/// 取得した画像を一時ファイルへ書き込む。
pub(super) async fn write_image<F: LocalFileClient>(
    local_file: &F,
    key: ImageKey,
    content: Vec<u8>,
) -> UsecaseOutput {
    let action = match local_file
        .write_temp_file(&temp_file_name(key), content)
        .await
    {
        Ok(path) => ImageAction::Loaded { key, path },
        Err(error) => ImageAction::Fail {
            key,
            reason: format!("failed to write {}: {}", error.path.display(), error.reason),
        },
    };
    vec![action.into()].into()
}

/// Redmine上のファイル名は使わず、画像の取得元だけから一時ファイルの名前を作る。
fn temp_file_name(key: ImageKey) -> String {
    match key {
        ImageKey::AttachmentThumbnail { id, size } => format!("attachment-{id}-thumbnail-{size}"),
        ImageKey::AttachmentOriginal { id } => format!("attachment-{id}-original"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use crate::stores::{Action, AttachmentImageAction};

    use super::super::image_test_support::{
        ImageStubClient, StubTempFileClient, TEMP_DIR, image_enabled_dispatcher, offline,
    };
    use super::*;

    const ATTACHMENT_ID: AttachmentId = AttachmentId::new(1);
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

    /// サムネイルを取得できず、原画像の取得へ切り替えた状態を作る。
    fn fallen_back_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = image_enabled_dispatcher();
        dispatcher.dispatch(AttachmentImageAction::Start {
            attachment_id: ATTACHMENT_ID,
            key: ImageKey::AttachmentThumbnail {
                id: ATTACHMENT_ID,
                size: 800,
            },
        });
        dispatcher.dispatch(AttachmentImageAction::FallBackToOriginal {
            attachment_id: ATTACHMENT_ID,
            reason: "not found".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.consume_action();
        Rc::new(RefCell::new(dispatcher))
    }

    #[tokio::test]
    async fn original_is_written_to_a_temporary_file() {
        let clients = clients(ImageStubClient::new(), StubTempFileClient::new());

        let output =
            download_attachment_original(fallen_back_dispatcher(), clients.clone(), ATTACHMENT_ID)
                .expect("download should start")
                .await;

        assert_eq!(
            *clients.redmine.content_gets.lock().unwrap(),
            vec![(ATTACHMENT_ID, 20_971_520)]
        );
        assert_eq!(
            *clients.local_file.writes.lock().unwrap(),
            vec![("attachment-1-original".to_string(), vec![2])]
        );
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Loaded { key, path })]
                if *key == ORIGINAL && path == &Path::new(TEMP_DIR).join("attachment-1-original")
        ));
    }

    #[tokio::test]
    async fn failed_get_fails_the_original() {
        let mut redmine = ImageStubClient::new();
        redmine.content_result = Err(offline());
        let clients = clients(redmine, StubTempFileClient::new());

        let output =
            download_attachment_original(fallen_back_dispatcher(), clients.clone(), ATTACHMENT_ID)
                .expect("download should start")
                .await;

        assert!(clients.local_file.writes.lock().unwrap().is_empty());
        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Fail { key, reason })]
                if *key == ORIGINAL && reason == "network error: offline"
        ));
    }

    #[tokio::test]
    async fn failed_write_fails_the_original() {
        let mut local_file = StubTempFileClient::new();
        local_file.fails = true;
        let clients = clients(ImageStubClient::new(), local_file);

        let output =
            download_attachment_original(fallen_back_dispatcher(), clients.clone(), ATTACHMENT_ID)
                .expect("download should start")
                .await;

        assert!(matches!(
            output.actions.as_slice(),
            [Action::Image(ImageAction::Fail { key, reason })]
                if *key == ORIGINAL
                    && reason == "failed to write /tmp/redmine-tui-test/attachment-1-original: disk full"
        ));
    }

    #[test]
    #[should_panic(
        expected = "cannot download the original image of attachment 1 while it uses None"
    )]
    fn downloading_without_falling_back_panics() {
        let clients = clients(ImageStubClient::new(), StubTempFileClient::new());

        download_attachment_original(
            Rc::new(RefCell::new(image_enabled_dispatcher())),
            clients,
            ATTACHMENT_ID,
        );
    }
}
