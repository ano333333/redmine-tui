use crate::vos::{IssueId, IssueStatusId, ProjectId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectsIssue {
    pub issue_id: IssueId,
    pub project_id: ProjectId,
    pub subject: String,
    pub description: String,
    pub status_id: IssueStatusId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIssuesPage {
    pub issues: Vec<ProjectsIssue>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
}
