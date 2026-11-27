//! 画像を取得する各段階のテストで共有するstub。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::clients::local_file::{LocalFileClient, LocalFileError};
use crate::clients::redmine::RedmineClientError;
use crate::stores::{Dispatcher, ImageDisplay, SettingsAction};
use crate::test_support::{RedmineClientStub, sample_png, test_image_encoder};
use crate::vos::AttachmentId;

pub(super) const TEMP_DIR: &str = "/tmp/redmine-tui-test";

pub(super) fn offline() -> RedmineClientError {
    RedmineClientError::Network {
        reason: "offline".to_string(),
    }
}

/// `ImageStubClient`が既定で返すサムネイル。
pub(super) fn thumbnail_png() -> Vec<u8> {
    sample_png(8, 8)
}

/// `ImageStubClient`が既定で返す原画像。サムネイルと区別できる大きさにする。
pub(super) fn original_png() -> Vec<u8> {
    sample_png(16, 16)
}

pub(super) fn image_enabled_dispatcher() -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(SettingsAction::SetImageDisplay(ImageDisplay::Enabled {
        encoder: test_image_encoder(),
    }));
    dispatcher.consume_action();
    dispatcher
}

pub(super) struct ImageStubClient {
    pub(super) thumbnail_result: Result<Vec<u8>, RedmineClientError>,
    pub(super) content_result: Result<Vec<u8>, RedmineClientError>,
    pub(super) thumbnail_gets: Mutex<Vec<(AttachmentId, u32, u64)>>,
    pub(super) content_gets: Mutex<Vec<(AttachmentId, u64)>>,
}

impl ImageStubClient {
    pub(super) fn new() -> Self {
        Self {
            thumbnail_result: Ok(thumbnail_png()),
            content_result: Ok(original_png()),
            thumbnail_gets: Mutex::new(Vec::new()),
            content_gets: Mutex::new(Vec::new()),
        }
    }
}

impl RedmineClientStub for ImageStubClient {
    async fn get_attachment_thumbnail(
        &self,
        id: AttachmentId,
        size: u32,
        max_len: u64,
    ) -> Result<Vec<u8>, RedmineClientError> {
        self.thumbnail_gets
            .lock()
            .unwrap()
            .push((id, size, max_len));
        self.thumbnail_result.clone()
    }

    async fn get_attachment_content(
        &self,
        id: AttachmentId,
        max_len: u64,
    ) -> Result<Vec<u8>, RedmineClientError> {
        self.content_gets.lock().unwrap().push((id, max_len));
        self.content_result.clone()
    }
}

/// 一時ファイルを`TEMP_DIR`の下に書いたことにして、書き込んだ内容を記録する。書き込んだファイルは読み込める。
pub(super) struct StubTempFileClient {
    pub(super) fails: bool,
    pub(super) writes: Mutex<Vec<(String, Vec<u8>)>>,
}

impl StubTempFileClient {
    pub(super) fn new() -> Self {
        Self {
            fails: false,
            writes: Mutex::new(Vec::new()),
        }
    }
}

impl LocalFileClient for StubTempFileClient {
    async fn read_file(&self, path: &Path, _: u64) -> Result<Vec<u8>, LocalFileError> {
        let writes = self.writes.lock().unwrap();
        let (_, content) = writes
            .iter()
            .rev()
            .find(|(file_name, _)| Path::new(TEMP_DIR).join(file_name) == path)
            .unwrap_or_else(|| panic!("{} was not written", path.display()));
        Ok(content.clone())
    }

    async fn write_temp_file(
        &self,
        file_name: &str,
        content: Vec<u8>,
    ) -> Result<PathBuf, LocalFileError> {
        self.writes
            .lock()
            .unwrap()
            .push((file_name.to_string(), content));
        let path = Path::new(TEMP_DIR).join(file_name);
        if self.fails {
            Err(LocalFileError {
                path,
                reason: "disk full".to_string(),
            })
        } else {
            Ok(path)
        }
    }
}
