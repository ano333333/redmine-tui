use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::clients::local_file::LocalFileClient;
use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, ImageAction, ImageDisplay, ImageKey, ImageState};
use crate::usecases::{UsecaseClients, UsecaseTask};

/// 前処理した画像の長辺の上限。これを超える画像は縮小し、前処理の時間とメモリを一定の範囲に収める。
pub const IMAGE_MAX_SIDE: u32 = 800;
/// 前処理する画像のファイルのバイト数の上限。
pub const IMAGE_FILE_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// 読み込んだ画像のファイルを読んでdecodeし、表示用にencodeする。
///
/// # Panics
///
/// 画像表示が無効な場合と、`key`の画像が`path`を前処理する状態でない場合にpanicする。
pub fn prepare_image<C, F>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    clients: UsecaseClients<C, F>,
    key: ImageKey,
    path: PathBuf,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
    F: LocalFileClient + Send + Sync + 'static,
{
    let encoder = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        let state = store.try_get_image(key);
        assert!(
            matches!(state, Some(ImageState::Preparing { path: preparing }) if *preparing == path),
            "cannot prepare image {key:?} from {} at {state:?}",
            path.display()
        );
        let ImageDisplay::Enabled { encoder } = store.get_image_display() else {
            panic!("cannot prepare image {key:?} while image display is disabled");
        };
        encoder.clone()
    };
    Some(Box::pin(async move {
        let action = match clients
            .local_file
            .read_file(&path, IMAGE_FILE_MAX_BYTES)
            .await
        {
            Ok(content) => match encoder.prepare(&content, IMAGE_MAX_SIDE) {
                Ok(image) => ImageAction::Prepared { key, image },
                Err(reason) => ImageAction::Fail { key, reason },
            },
            Err(error) => ImageAction::Fail {
                key,
                reason: error.to_string(),
            },
        };
        vec![action.into()].into()
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use ratatui::layout::Size;

    use super::*;
    use crate::clients::local_file::LocalFileError;
    use crate::stores::{Action, SettingsAction};
    use crate::test_support::{UnusedRedmineClient, sample_png, test_image_encoder};

    const KEY: ImageKey = ImageKey::AttachmentOriginal {
        id: crate::vos::AttachmentId::new(1),
    };
    const PATH: &str = "/tmp/redmine-tui-test/attachment-1-original";

    struct StubFileClient {
        result: Result<Vec<u8>, LocalFileError>,
        reads: Mutex<Vec<(PathBuf, u64)>>,
    }

    impl LocalFileClient for StubFileClient {
        async fn read_file(&self, path: &Path, max_len: u64) -> Result<Vec<u8>, LocalFileError> {
            self.reads
                .lock()
                .unwrap()
                .push((path.to_path_buf(), max_len));
            self.result.clone()
        }

        async fn write_temp_file(&self, _: &str, _: Vec<u8>) -> Result<PathBuf, LocalFileError> {
            unreachable!("preparing does not write temporary files")
        }
    }

    /// 画像表示を有効にし、`KEY`の画像が`PATH`を前処理する状態を作る。
    fn preparing_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(SettingsAction::SetImageDisplay(ImageDisplay::Enabled {
            encoder: test_image_encoder(),
        }));
        dispatcher.dispatch(ImageAction::Start { key: KEY });
        dispatcher.dispatch(ImageAction::Loaded {
            key: KEY,
            path: PathBuf::from(PATH),
        });
        while dispatcher.consume_actinos_len() > 0 {
            dispatcher.consume_action();
        }
        Rc::new(RefCell::new(dispatcher))
    }

    fn clients(
        result: Result<Vec<u8>, LocalFileError>,
    ) -> UsecaseClients<UnusedRedmineClient, StubFileClient> {
        UsecaseClients {
            redmine: Arc::new(UnusedRedmineClient),
            local_file: Arc::new(StubFileClient {
                result,
                reads: Mutex::new(Vec::new()),
            }),
        }
    }

    async fn prepare(clients: &UsecaseClients<UnusedRedmineClient, StubFileClient>) -> Vec<Action> {
        prepare_image(
            preparing_dispatcher(),
            clients.clone(),
            KEY,
            PathBuf::from(PATH),
        )
        .expect("preparing should start")
        .await
        .actions
    }

    #[tokio::test]
    async fn small_image_is_prepared_at_its_original_size() {
        let clients = clients(Ok(sample_png(16, 32)));

        let actions = prepare(&clients).await;

        assert_eq!(
            *clients.local_file.reads.lock().unwrap(),
            vec![(PathBuf::from(PATH), 20_971_520)]
        );
        // テスト用のencoderのフォントは10x20pxのため、16x32pxは2x2セルになる。
        assert!(matches!(
            actions.as_slice(),
            [Action::Image(ImageAction::Prepared { key, image })]
                if *key == KEY && image.pixel_size() == (16, 32) && image.size() == Size::new(2, 2)
        ));
    }

    #[tokio::test]
    async fn image_longer_than_the_limit_is_shrunk() {
        let clients = clients(Ok(sample_png(1600, 400)));

        let actions = prepare(&clients).await;

        assert!(matches!(
            actions.as_slice(),
            [Action::Image(ImageAction::Prepared { image, .. })]
                if image.pixel_size() == (800, 200)
        ));
    }

    #[tokio::test]
    async fn undecodable_file_fails_the_image() {
        let clients = clients(Ok(b"not an image".to_vec()));

        let actions = prepare(&clients).await;

        assert!(matches!(
            actions.as_slice(),
            [Action::Image(ImageAction::Fail { key, .. })] if *key == KEY
        ));
    }

    #[tokio::test]
    async fn unreadable_file_fails_the_image() {
        let clients = clients(Err(LocalFileError {
            path: PathBuf::from(PATH),
            reason: "missing".to_string(),
        }));

        let actions = prepare(&clients).await;

        assert!(matches!(
            actions.as_slice(),
            [Action::Image(ImageAction::Fail { key, reason })]
                if *key == KEY
                    && reason == "failed to read /tmp/redmine-tui-test/attachment-1-original: missing"
        ));
    }

    #[test]
    #[should_panic(expected = "cannot prepare image AttachmentOriginal")]
    fn preparing_an_image_that_is_not_loaded_panics() {
        prepare_image(
            Rc::new(RefCell::new(Dispatcher::new())),
            clients(Ok(sample_png(1, 1))),
            KEY,
            PathBuf::from(PATH),
        );
    }
}
