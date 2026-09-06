use std::num::NonZeroUsize;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::clients::redmine::{RedmineClient, RedmineClientError, RedmineHttpError};
use crate::entities::{
    Category, Issue, IssueAggregate, IssueStatus, Journal, Priority, Project, ProjectIssuesPage,
    TargetVersion, TimeEntityActivity, Tracker, User,
};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssueStatusId, JournalId, JournalKey, PriorityId,
    ProjectId, TargetVersionId, TimeEntityActivityId, TrackerId, UserId,
};

mod journal_detail_conversion;
mod journal_detail_value_conversion;

// FIXME: ユーザーを全列挙しないことを前提としたStore管理
const PAGE_LIMIT: usize = 100;
const PROJECT_ISSUES_PAGE_LIMIT: usize = 50;

pub struct DefaultRedmineClient {
    host_url: String,
    access_token: String,
    http_client: reqwest::Client,
}

impl DefaultRedmineClient {
    pub fn new(host_url: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            host_url: host_url.into().trim_end_matches('/').to_string(),
            access_token: access_token.into(),
            http_client: reqwest::Client::new(),
        }
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, RedmineClientError> {
        let method = "GET";
        let url = format!("{}{}", self.host_url, path);
        let response = self
            .http_client
            .get(&url)
            .header("X-Redmine-API-Key", &self.access_token)
            .send()
            .await
            .map_err(map_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let response_body = response.text().await.unwrap_or_else(|error| {
                format!("failed to read Redmine error response body: {error}")
            });
            return map_response_status(
                status,
                RedmineHttpError {
                    method: method.to_string(),
                    url,
                    status_code: status.as_u16(),
                    response_body,
                },
            );
        }

        response.json().await.map_err(map_response_body_error)
    }

    async fn put_empty<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<(), RedmineClientError> {
        let method = "PUT";
        let url = format!("{}{}", self.host_url, path);
        let response = self
            .http_client
            .put(&url)
            .header("X-Redmine-API-Key", &self.access_token)
            .json(body)
            .send()
            .await
            .map_err(map_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let response_body = response.text().await.unwrap_or_else(|error| {
                format!("failed to read Redmine error response body: {error}")
            });
            return map_response_status(
                status,
                RedmineHttpError {
                    method: method.to_string(),
                    url,
                    status_code: status.as_u16(),
                    response_body,
                },
            );
        }

        Ok(())
    }

    async fn get_paginated<R>(&self, path: &str) -> Result<Vec<R::Item>, RedmineClientError>
    where
        R: DeserializeOwned + PaginatedResponse,
    {
        let mut offset = 0;
        let mut items = Vec::new();

        loop {
            let separator = if path.contains('?') { '&' } else { '?' };
            let page_path = format!("{path}{separator}limit={PAGE_LIMIT}&offset={offset}");
            let response: R = self.get_json(&page_path).await?;
            let (mut page_items, page_info) = response.into_parts();
            let page_item_count = page_items.len();

            items.append(&mut page_items);

            if page_info.is_last_page(offset, page_item_count) {
                break;
            }

            offset += page_item_count;
        }

        Ok(items)
    }
}

impl RedmineClient for DefaultRedmineClient {
    async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
        let mut categories = Vec::new();

        for project in self.get_projects().await? {
            let path = format!("/projects/{}/issue_categories.json", project.id.get());
            categories.extend(self.get_paginated::<IssueCategoriesResponse>(&path).await?);
        }

        Ok(categories)
    }

    async fn get_issue(
        &self,
        id: IssueId,
    ) -> Result<(IssueAggregate, Vec<Journal>), RedmineClientError> {
        let mut response: IssueResponse = self
            .get_json(&format!("/issues/{id}.json?include=children,journals"))
            .await?;

        let journals = std::mem::take(&mut response.issue.journals)
            .into_iter()
            .map(Journal::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let journal_keys = journals
            .iter()
            .map(|journal| JournalKey::Remote(journal.id))
            .collect();
        let mut issue: IssueAggregate = response.issue.try_into()?;
        issue.journal_keys = journal_keys;
        Ok((issue, journals))
    }

    async fn create_journal(
        &self,
        issue_id: IssueId,
        notes: &str,
    ) -> Result<(), RedmineClientError> {
        self.put_empty(
            &format!("/issues/{}.json", issue_id.get()),
            &CreateJournalRequest {
                issue: CreateJournal { notes },
            },
        )
        .await
    }

    async fn update_issue(&self, issue: &IssueAggregate) -> Result<(), RedmineClientError> {
        self.put_empty(
            &format!("/issues/{}.json", issue.issue.id.get()),
            &UpdateIssueRequest::from(issue),
        )
        .await
    }

    async fn update_journal_notes(
        &self,
        id: JournalId,
        notes: &str,
    ) -> Result<(), RedmineClientError> {
        self.put_empty(
            &format!("/journals/{}.json", id.get()),
            &UpdateJournalNotesRequest {
                journal: UpdateJournalNotes { notes },
            },
        )
        .await
    }

    async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError> {
        Ok(self
            .get_json::<IssueStatusesResponse>("/issue_statuses.json")
            .await?
            .issue_statuses
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError> {
        Ok(self
            .get_json::<PrioritiesResponse>("/enumerations/issue_priorities.json")
            .await?
            .issue_priorities
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError> {
        self.get_paginated::<ProjectsResponse>("/projects.json")
            .await
    }

    async fn get_project_issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> Result<ProjectIssuesPage, RedmineClientError> {
        let expected_offset = page
            .get()
            .checked_sub(1)
            .and_then(|value| value.checked_mul(PROJECT_ISSUES_PAGE_LIMIT))
            .ok_or_else(|| RedmineClientError::Client {
                reason: format!("project issue page {} has an invalid offset", page.get()),
            })?;
        let response: ProjectIssuesResponse = self
            .get_json(&format!(
                "/issues.json?project_id={}&status_id=*&sort=id:desc&limit={PROJECT_ISSUES_PAGE_LIMIT}&page={}",
                project_id.get(),
                page.get(),
            ))
            .await?;

        if response.limit != PROJECT_ISSUES_PAGE_LIMIT {
            return Err(RedmineClientError::Client {
                reason: format!(
                    "project issues response limit {} does not match requested limit {PROJECT_ISSUES_PAGE_LIMIT}",
                    response.limit
                ),
            });
        }
        if response.offset != expected_offset {
            return Err(RedmineClientError::Client {
                reason: format!(
                    "project issues response offset {} does not match requested offset {expected_offset}",
                    response.offset
                ),
            });
        }

        let issues: Vec<Issue> = response.issues.into_iter().map(Into::into).collect();
        if issues.iter().any(|issue| issue.project_id != project_id) {
            return Err(RedmineClientError::Client {
                reason: format!(
                    "project issues response contains an issue outside requested project {}",
                    project_id.get()
                ),
            });
        }

        Ok(ProjectIssuesPage {
            issues,
            total_count: response.total_count,
            offset: response.offset,
            limit: response.limit,
        })
    }

    async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError> {
        let mut versions = Vec::new();

        for project in self.get_projects().await? {
            let path = format!("/projects/{}/versions.json", project.id.get());
            versions.extend(self.get_paginated::<VersionsResponse>(&path).await?);
        }

        Ok(versions)
    }

    async fn get_time_entity_activities(
        &self,
    ) -> Result<Vec<TimeEntityActivity>, RedmineClientError> {
        Ok(self
            .get_json::<TimeEntryActivitiesResponse>("/enumerations/time_entry_activities.json")
            .await?
            .time_entry_activities
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError> {
        Ok(self
            .get_json::<TrackersResponse>("/trackers.json")
            .await?
            .trackers
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn get_users(&self) -> Result<Vec<User>, RedmineClientError> {
        self.get_paginated::<UsersResponse>("/users.json").await
    }
}

fn map_response_status<T>(
    status: StatusCode,
    context: RedmineHttpError,
) -> Result<T, RedmineClientError> {
    match status.as_u16() {
        400 => Err(RedmineClientError::BadRequest { context }),
        401 => Err(RedmineClientError::Unauthorized { context }),
        403 | 404 => Err(RedmineClientError::NotFound { context }),
        422 => Err(RedmineClientError::UnprocessableEntity { context }),
        500..=599 => Err(RedmineClientError::InternalServerError { context }),
        _ => panic!(
            "unexpected Redmine response status: {} for {} {} with body: {}",
            status.as_u16(),
            context.method,
            context.url,
            context.response_body
        ),
    }
}

fn map_request_error(error: reqwest::Error) -> RedmineClientError {
    if error.is_builder() {
        RedmineClientError::Client {
            reason: error.to_string(),
        }
    } else {
        RedmineClientError::Network {
            reason: error.to_string(),
        }
    }
}

fn map_response_body_error(error: reqwest::Error) -> RedmineClientError {
    if error.is_decode() {
        RedmineClientError::Client {
            reason: error.to_string(),
        }
    } else {
        RedmineClientError::Network {
            reason: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct PageInfo {
    #[serde(default)]
    total_count: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

impl PageInfo {
    fn is_last_page(self, offset: usize, page_item_count: usize) -> bool {
        if page_item_count == 0 {
            return true;
        }

        let limit = self.limit.unwrap_or(PAGE_LIMIT);
        if page_item_count < limit {
            return true;
        }

        self.total_count
            .map(|total_count| offset + page_item_count >= total_count)
            .unwrap_or(true)
    }
}

trait PaginatedResponse {
    type Item;

    fn into_parts(self) -> (Vec<Self::Item>, PageInfo);
}

#[derive(Deserialize)]
struct NamedRedmineEntity {
    id: u16,
    name: String,
}

impl From<NamedRedmineEntity> for Category {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: CategoryId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for Priority {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: PriorityId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for Project {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: ProjectId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for TargetVersion {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: TargetVersionId::new(value.id),
            name: value.name,
        }
    }
}

impl From<NamedRedmineEntity> for Tracker {
    fn from(value: NamedRedmineEntity) -> Self {
        Self {
            id: TrackerId::new(value.id),
            name: value.name,
        }
    }
}

#[derive(Deserialize)]
struct RedmineIssueStatus {
    id: u16,
    name: String,
    is_closed: bool,
}

impl From<RedmineIssueStatus> for IssueStatus {
    fn from(value: RedmineIssueStatus) -> Self {
        Self {
            id: IssueStatusId::new(value.id),
            name: value.name,
            is_closed: value.is_closed,
        }
    }
}

#[derive(Deserialize)]
struct RedmineTimeEntryActivity {
    id: u16,
    name: String,
    is_default: bool,
}

impl From<RedmineTimeEntryActivity> for TimeEntityActivity {
    fn from(value: RedmineTimeEntryActivity) -> Self {
        Self {
            id: TimeEntityActivityId::new(value.id),
            name: value.name,
            is_default: value.is_default,
        }
    }
}

#[derive(Deserialize)]
struct RedmineUser {
    id: u16,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    firstname: Option<String>,
    #[serde(default)]
    lastname: Option<String>,
}

impl RedmineUser {
    fn display_name(self) -> String {
        if let Some(name) = self.name {
            return name;
        }

        match (self.firstname, self.lastname) {
            (Some(firstname), Some(lastname)) if !lastname.is_empty() => {
                format!("{firstname} {lastname}")
            }
            (Some(firstname), _) => firstname,
            (_, Some(lastname)) => lastname,
            _ => String::new(),
        }
    }
}

impl From<RedmineUser> for User {
    fn from(value: RedmineUser) -> Self {
        Self {
            id: UserId::new(value.id),
            name: value.display_name(),
        }
    }
}

#[derive(Deserialize)]
struct UsersResponse {
    users: Vec<RedmineUser>,
    #[serde(flatten)]
    page_info: PageInfo,
}

impl PaginatedResponse for UsersResponse {
    type Item = User;

    fn into_parts(self) -> (Vec<Self::Item>, PageInfo) {
        (
            self.users.into_iter().map(Into::into).collect(),
            self.page_info,
        )
    }
}

#[derive(Deserialize)]
struct ProjectsResponse {
    projects: Vec<NamedRedmineEntity>,
    #[serde(flatten)]
    page_info: PageInfo,
}

impl PaginatedResponse for ProjectsResponse {
    type Item = Project;

    fn into_parts(self) -> (Vec<Self::Item>, PageInfo) {
        (
            self.projects.into_iter().map(Into::into).collect(),
            self.page_info,
        )
    }
}

#[derive(Deserialize)]
struct IssueCategoriesResponse {
    issue_categories: Vec<NamedRedmineEntity>,
    #[serde(flatten)]
    page_info: PageInfo,
}

impl PaginatedResponse for IssueCategoriesResponse {
    type Item = Category;

    fn into_parts(self) -> (Vec<Self::Item>, PageInfo) {
        (
            self.issue_categories.into_iter().map(Into::into).collect(),
            self.page_info,
        )
    }
}

#[derive(Deserialize)]
struct VersionsResponse {
    versions: Vec<NamedRedmineEntity>,
    #[serde(flatten)]
    page_info: PageInfo,
}

impl PaginatedResponse for VersionsResponse {
    type Item = TargetVersion;

    fn into_parts(self) -> (Vec<Self::Item>, PageInfo) {
        (
            self.versions.into_iter().map(Into::into).collect(),
            self.page_info,
        )
    }
}

#[derive(Deserialize)]
struct IssueStatusesResponse {
    issue_statuses: Vec<RedmineIssueStatus>,
}

#[derive(Deserialize)]
struct PrioritiesResponse {
    issue_priorities: Vec<NamedRedmineEntity>,
}

#[derive(Deserialize)]
struct TimeEntryActivitiesResponse {
    time_entry_activities: Vec<RedmineTimeEntryActivity>,
}

#[derive(Deserialize)]
struct TrackersResponse {
    trackers: Vec<NamedRedmineEntity>,
}

#[derive(Deserialize)]
struct IssueResponse {
    issue: RedmineIssue,
}

#[derive(Deserialize)]
struct ProjectIssuesResponse {
    issues: Vec<RedmineProjectIssue>,
    total_count: usize,
    offset: usize,
    limit: usize,
}

#[derive(Deserialize)]
struct RedmineProjectIssue {
    id: u16,
    project: RedmineIdRef,
    subject: String,
    #[serde(default)]
    description: Option<String>,
    status: RedmineIdRef,
}

impl From<RedmineProjectIssue> for Issue {
    fn from(value: RedmineProjectIssue) -> Self {
        Self {
            id: IssueId::new(value.id),
            project_id: ProjectId::new(value.project.id),
            subject: value.subject,
            description: value.description.unwrap_or_default(),
            status_id: IssueStatusId::new(value.status.id),
        }
    }
}

#[derive(Serialize)]
struct UpdateIssueRequest {
    issue: UpdateIssue,
}

#[derive(Serialize)]
struct CreateJournalRequest<'a> {
    issue: CreateJournal<'a>,
}

#[derive(Serialize)]
struct CreateJournal<'a> {
    notes: &'a str,
}

#[derive(Serialize)]
struct UpdateJournalNotesRequest<'a> {
    journal: UpdateJournalNotes<'a>,
}

#[derive(Serialize)]
struct UpdateJournalNotes<'a> {
    notes: &'a str,
}

impl From<&IssueAggregate> for UpdateIssueRequest {
    fn from(issue: &IssueAggregate) -> Self {
        Self {
            issue: UpdateIssue::from(issue),
        }
    }
}

#[derive(Serialize)]
struct UpdateIssue {
    subject: String,
    description: String,
    status_id: u16,
    priority_id: u16,
    assigned_to_id: Option<u16>,
    fixed_version_id: Option<u16>,
    start_date: Option<String>,
    due_date: Option<String>,
    done_ratio: u16,
    estimated_hours: Option<u16>,
    category_id: Option<u16>,
}

impl From<&IssueAggregate> for UpdateIssue {
    fn from(issue: &IssueAggregate) -> Self {
        Self {
            subject: issue.issue.subject.clone(),
            description: issue.issue.description.clone(),
            status_id: issue.issue.status_id.get(),
            priority_id: issue.priority_id.get(),
            assigned_to_id: issue.assigned_to_id.map(|id| id.get()),
            fixed_version_id: issue.target_version_id.map(|id| id.get()),
            start_date: issue
                .start_date
                .map(|date| date.format("%Y-%m-%d").to_string()),
            due_date: issue
                .due_date
                .map(|date| date.format("%Y-%m-%d").to_string()),
            done_ratio: issue.done_ratio,
            estimated_hours: issue.estimated_hours,
            category_id: issue.category_id.map(|id| id.get()),
        }
    }
}

#[derive(Deserialize)]
struct RedmineIssue {
    id: u16,
    subject: String,
    author: RedmineIdRef,
    created_on: String,
    updated_on: String,
    project: RedmineIdRef,
    tracker: RedmineIdRef,
    status: RedmineIdRef,
    priority: RedmineIdRef,
    #[serde(default)]
    assigned_to: Option<RedmineIdRef>,
    #[serde(default)]
    fixed_version: Option<RedmineIdRef>,
    #[serde(default)]
    start_date: Option<String>,
    #[serde(default)]
    due_date: Option<String>,
    done_ratio: u16,
    #[serde(default)]
    estimated_hours: Option<f64>,
    #[serde(default)]
    total_spent_hours: Option<f64>,
    #[serde(default)]
    category: Option<RedmineIdRef>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    children: Vec<RedmineIdRef>,
    #[serde(default)]
    journals: Vec<RedmineJournal>,
}

#[derive(Deserialize)]
struct RedmineJournal {
    id: u16,
    user: NamedRedmineEntity,
    updated_on: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    details: Vec<journal_detail_conversion::RedmineJournalDetail>,
}

impl TryFrom<RedmineJournal> for Journal {
    type Error = RedmineClientError;

    fn try_from(value: RedmineJournal) -> Result<Self, Self::Error> {
        Ok(Self {
            id: JournalId::new(value.id),
            user: value.user.name,
            updated_on: journal_detail_value_conversion::parse_timestamp(&value.updated_on)?,
            notes: value.notes,
            details: value
                .details
                .into_iter()
                .filter_map(|detail| detail.try_into_domain().transpose())
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<RedmineIssue> for IssueAggregate {
    type Error = RedmineClientError;

    fn try_from(value: RedmineIssue) -> Result<Self, Self::Error> {
        let issue = Issue {
            id: IssueId::new(value.id),
            project_id: ProjectId::new(value.project.id),
            subject: value.subject.clone(),
            description: value.description.clone().unwrap_or_default(),
            status_id: IssueStatusId::new(value.status.id),
        };

        Ok(Self {
            author_id: UserId::new(value.author.id),
            created_on: journal_detail_value_conversion::parse_timestamp(&value.created_on)?,
            updated_on: journal_detail_value_conversion::parse_timestamp(&value.updated_on)?,
            tracker_id: TrackerId::new(value.tracker.id),
            priority_id: PriorityId::new(value.priority.id),
            assigned_to_id: value
                .assigned_to
                .map(|assigned_to| UserId::new(assigned_to.id)),
            target_version_id: value
                .fixed_version
                .map(|fixed_version| TargetVersionId::new(fixed_version.id)),
            start_date: journal_detail_value_conversion::parse_optional_calendar_date(
                value.start_date.as_deref(),
            )?,
            due_date: journal_detail_value_conversion::parse_optional_calendar_date(
                value.due_date.as_deref(),
            )?,
            done_ratio: value.done_ratio,
            estimated_hours: value.estimated_hours.map(|hours| hours as u16),
            total_spent_hours: value.total_spent_hours,
            category_id: value.category.map(|category| CategoryId::new(category.id)),
            child_ids: value
                .children
                .into_iter()
                .map(|child| IssueId::new(child.id))
                .collect(),
            journal_keys: value
                .journals
                .into_iter()
                .map(|journal| JournalKey::Remote(JournalId::new(journal.id)))
                .collect(),
            issue,
        })
    }
}

#[derive(Deserialize)]
struct RedmineIdRef {
    id: u16,
}

#[cfg(test)]
#[path = "default_tests/mod.rs"]
mod tests;
