//! 画像を取得する各段階のテストで共有するstub。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::clients::local_file::{LocalFileClient, LocalFileError};
use crate::clients::redmine::base::FetchedIssue;
use crate::clients::redmine::{IssueUpdate, RedmineClient, RedmineClientError};
use crate::entities::{
    Category, IssueStatus, Priority, Project, ProjectIssuesPage, TargetVersion, TimeEntityActivity,
    Tracker, User,
};
use crate::stores::{Dispatcher, ImageDisplay, SettingsAction};
use crate::test_support::{sample_png, test_image_encoder};
use crate::vos::{AttachmentId, IssueId, JournalId, ProjectId, UploadToken};

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

/// 添付ファイルの取得結果を返し、取得した添付ファイルを記録する。
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

impl RedmineClient for ImageStubClient {
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

    async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
        unreachable!()
    }

    async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
        unreachable!()
    }

    async fn update_issue(&self, _: IssueId, _: &IssueUpdate) -> Result<(), RedmineClientError> {
        unreachable!()
    }

    async fn update_journal_notes(&self, _: JournalId, _: &str) -> Result<(), RedmineClientError> {
        unreachable!()
    }

    async fn update_issue_notes(&self, _: IssueId, _: &str) -> Result<(), RedmineClientError> {
        unreachable!()
    }

    async fn upload_file(&self, _: &str, _: Vec<u8>) -> Result<UploadToken, RedmineClientError> {
        unreachable!()
    }

    async fn attach_upload(
        &self,
        _: IssueId,
        _: &UploadToken,
        _: &str,
    ) -> Result<(), RedmineClientError> {
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
        _: ProjectId,
        _: std::num::NonZeroUsize,
    ) -> Result<ProjectIssuesPage, RedmineClientError> {
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
