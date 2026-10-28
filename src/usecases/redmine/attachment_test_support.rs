//! 添付ファイル追加の各段階のテストで共有するstub。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::clients::local_file::{LocalFileClient, LocalFileError};
use crate::clients::redmine::base::FetchedIssue;
use crate::clients::redmine::{IssueUpdate, RedmineClient, RedmineClientError};
use crate::entities::{
    Category, IssueStatus, Priority, Project, ProjectIssuesPage, TargetVersion, TimeEntityActivity,
    Tracker, User,
};
use crate::vos::{IssueId, JournalId, ProjectId, UploadToken};

pub(super) fn offline() -> RedmineClientError {
    RedmineClientError::Network {
        reason: "offline".to_string(),
    }
}

/// 添付ファイル追加で使うAPIの結果を返し、送った内容を記録する。
pub(super) struct AttachmentStubClient {
    pub(super) upload_result: Result<UploadToken, RedmineClientError>,
    pub(super) attach_result: Result<(), RedmineClientError>,
    pub(super) get_results: Mutex<VecDeque<Result<FetchedIssue, RedmineClientError>>>,
    pub(super) uploads: Mutex<Vec<(String, Vec<u8>)>>,
    pub(super) attaches: Mutex<Vec<(IssueId, UploadToken, String)>>,
}

impl AttachmentStubClient {
    pub(super) fn new() -> Self {
        Self {
            upload_result: Ok(UploadToken::new("7.token")),
            attach_result: Ok(()),
            get_results: Mutex::new(VecDeque::new()),
            uploads: Mutex::new(Vec::new()),
            attaches: Mutex::new(Vec::new()),
        }
    }
}

impl RedmineClient for AttachmentStubClient {
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

    async fn attach_upload(
        &self,
        issue_id: IssueId,
        token: &UploadToken,
        filename: &str,
    ) -> Result<(), RedmineClientError> {
        self.attaches
            .lock()
            .unwrap()
            .push((issue_id, token.clone(), filename.to_string()));
        self.attach_result.clone()
    }

    async fn get_issue(&self, _: IssueId) -> Result<FetchedIssue, RedmineClientError> {
        self.get_results
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected GET")
    }

    async fn update_issue(&self, _: IssueId, _: &IssueUpdate) -> Result<(), RedmineClientError> {
        unreachable!()
    }

    async fn update_issue_notes(&self, _: IssueId, _: &str) -> Result<(), RedmineClientError> {
        unreachable!()
    }

    async fn update_journal_notes(&self, _: JournalId, _: &str) -> Result<(), RedmineClientError> {
        unreachable!()
    }

    async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
        unreachable!()
    }

    async fn get_attachment_thumbnail(
        &self,
        _: crate::vos::AttachmentId,
        _: u32,
        _: u64,
    ) -> Result<Vec<u8>, RedmineClientError> {
        unreachable!()
    }

    async fn get_attachment_content(
        &self,
        _: crate::vos::AttachmentId,
        _: u64,
    ) -> Result<Vec<u8>, RedmineClientError> {
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

/// 読み込み結果を返し、読み込んだパスと上限を記録する。
pub(super) struct StubLocalFileClient {
    pub(super) result: Result<Vec<u8>, LocalFileError>,
    pub(super) reads: Mutex<Vec<(PathBuf, u64)>>,
}

impl StubLocalFileClient {
    pub(super) fn new(result: Result<Vec<u8>, LocalFileError>) -> Self {
        Self {
            result,
            reads: Mutex::new(Vec::new()),
        }
    }
}

impl LocalFileClient for StubLocalFileClient {
    async fn read_file(&self, path: &Path, max_len: u64) -> Result<Vec<u8>, LocalFileError> {
        self.reads
            .lock()
            .unwrap()
            .push((path.to_path_buf(), max_len));
        self.result.clone()
    }

    async fn write_temp_file(&self, _: &str, _: Vec<u8>) -> Result<PathBuf, LocalFileError> {
        unreachable!("attachment upload does not write temporary files")
    }
}
