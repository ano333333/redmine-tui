//! 添付ファイル追加の各段階のテストで共有するstub。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::clients::local_file::{LocalFileClient, LocalFileError};
use crate::clients::redmine::RedmineClientError;
use crate::clients::redmine::base::FetchedIssue;
use crate::test_support::RedmineClientStub;
use crate::vos::{IssueId, UploadToken};

pub(super) fn offline() -> RedmineClientError {
    RedmineClientError::Network {
        reason: "offline".to_string(),
    }
}

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

impl RedmineClientStub for AttachmentStubClient {
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
}

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
