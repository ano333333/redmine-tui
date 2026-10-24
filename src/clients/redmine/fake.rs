//! テストで使うメソッドだけを実装すれば`RedmineClient`になるfake。

use std::future::Future;
use std::num::NonZeroUsize;

use super::base::FetchedIssue;
use super::{IssueUpdate, RedmineClient, RedmineClientError};
use crate::entities::{
    Category, IssueStatus, Priority, Project, ProjectIssuesPage, TargetVersion, TimeEntityActivity,
    Tracker, User,
};
use crate::vos::{IssueId, JournalId, ProjectId, UploadToken};

/// 実装しなかったメソッドは、呼ばれるとpanicする。
pub trait FakeRedmineClient {
    fn get_categories(
        &self,
    ) -> impl Future<Output = Result<Vec<Category>, RedmineClientError>> + Send {
        async { unexpected_call("get_categories") }
    }
    fn get_issue(
        &self,
        _: IssueId,
    ) -> impl Future<Output = Result<FetchedIssue, RedmineClientError>> + Send {
        async { unexpected_call("get_issue") }
    }
    fn update_issue(
        &self,
        _: IssueId,
        _: &IssueUpdate,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        async { unexpected_call("update_issue") }
    }
    fn update_journal_notes(
        &self,
        _: JournalId,
        _: &str,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        async { unexpected_call("update_journal_notes") }
    }
    fn update_issue_notes(
        &self,
        _: IssueId,
        _: &str,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        async { unexpected_call("update_issue_notes") }
    }
    fn upload_file(
        &self,
        _: &str,
        _: Vec<u8>,
    ) -> impl Future<Output = Result<UploadToken, RedmineClientError>> + Send {
        async { unexpected_call("upload_file") }
    }
    fn attach_upload(
        &self,
        _: IssueId,
        _: &UploadToken,
        _: &str,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        async { unexpected_call("attach_upload") }
    }
    fn get_issue_statuses(
        &self,
    ) -> impl Future<Output = Result<Vec<IssueStatus>, RedmineClientError>> + Send {
        async { unexpected_call("get_issue_statuses") }
    }
    fn get_priorities(
        &self,
    ) -> impl Future<Output = Result<Vec<Priority>, RedmineClientError>> + Send {
        async { unexpected_call("get_priorities") }
    }
    fn get_projects(
        &self,
    ) -> impl Future<Output = Result<Vec<Project>, RedmineClientError>> + Send {
        async { unexpected_call("get_projects") }
    }
    fn get_project_issues(
        &self,
        _: ProjectId,
        _: NonZeroUsize,
    ) -> impl Future<Output = Result<ProjectIssuesPage, RedmineClientError>> + Send {
        async { unexpected_call("get_project_issues") }
    }
    fn get_target_versions(
        &self,
    ) -> impl Future<Output = Result<Vec<TargetVersion>, RedmineClientError>> + Send {
        async { unexpected_call("get_target_versions") }
    }
    fn get_time_entity_activities(
        &self,
    ) -> impl Future<Output = Result<Vec<TimeEntityActivity>, RedmineClientError>> + Send {
        async { unexpected_call("get_time_entity_activities") }
    }
    fn get_trackers(
        &self,
    ) -> impl Future<Output = Result<Vec<Tracker>, RedmineClientError>> + Send {
        async { unexpected_call("get_trackers") }
    }
    fn get_users(&self) -> impl Future<Output = Result<Vec<User>, RedmineClientError>> + Send {
        async { unexpected_call("get_users") }
    }
}

fn unexpected_call<T>(method: &str) -> T {
    panic!("unexpected RedmineClient::{method} call")
}

impl<T: FakeRedmineClient> RedmineClient for T {
    fn get_categories(
        &self,
    ) -> impl Future<Output = Result<Vec<Category>, RedmineClientError>> + Send {
        FakeRedmineClient::get_categories(self)
    }
    fn get_issue(
        &self,
        id: IssueId,
    ) -> impl Future<Output = Result<FetchedIssue, RedmineClientError>> + Send {
        FakeRedmineClient::get_issue(self, id)
    }
    fn update_issue(
        &self,
        issue_id: IssueId,
        update: &IssueUpdate,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        FakeRedmineClient::update_issue(self, issue_id, update)
    }
    fn update_journal_notes(
        &self,
        journal_id: JournalId,
        notes: &str,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        FakeRedmineClient::update_journal_notes(self, journal_id, notes)
    }
    fn update_issue_notes(
        &self,
        issue_id: IssueId,
        notes: &str,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        FakeRedmineClient::update_issue_notes(self, issue_id, notes)
    }
    fn upload_file(
        &self,
        filename: &str,
        content: Vec<u8>,
    ) -> impl Future<Output = Result<UploadToken, RedmineClientError>> + Send {
        FakeRedmineClient::upload_file(self, filename, content)
    }
    fn attach_upload(
        &self,
        issue_id: IssueId,
        token: &UploadToken,
        filename: &str,
    ) -> impl Future<Output = Result<(), RedmineClientError>> + Send {
        FakeRedmineClient::attach_upload(self, issue_id, token, filename)
    }
    fn get_issue_statuses(
        &self,
    ) -> impl Future<Output = Result<Vec<IssueStatus>, RedmineClientError>> + Send {
        FakeRedmineClient::get_issue_statuses(self)
    }
    fn get_priorities(
        &self,
    ) -> impl Future<Output = Result<Vec<Priority>, RedmineClientError>> + Send {
        FakeRedmineClient::get_priorities(self)
    }
    fn get_projects(
        &self,
    ) -> impl Future<Output = Result<Vec<Project>, RedmineClientError>> + Send {
        FakeRedmineClient::get_projects(self)
    }
    fn get_project_issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> impl Future<Output = Result<ProjectIssuesPage, RedmineClientError>> + Send {
        FakeRedmineClient::get_project_issues(self, project_id, page)
    }
    fn get_target_versions(
        &self,
    ) -> impl Future<Output = Result<Vec<TargetVersion>, RedmineClientError>> + Send {
        FakeRedmineClient::get_target_versions(self)
    }
    fn get_time_entity_activities(
        &self,
    ) -> impl Future<Output = Result<Vec<TimeEntityActivity>, RedmineClientError>> + Send {
        FakeRedmineClient::get_time_entity_activities(self)
    }
    fn get_trackers(
        &self,
    ) -> impl Future<Output = Result<Vec<Tracker>, RedmineClientError>> + Send {
        FakeRedmineClient::get_trackers(self)
    }
    fn get_users(&self) -> impl Future<Output = Result<Vec<User>, RedmineClientError>> + Send {
        FakeRedmineClient::get_users(self)
    }
}
