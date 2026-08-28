use std::collections::HashMap;
use std::num::NonZeroUsize;

use crate::entities::{ProjectIssuesPage, ProjectsIssue};
use crate::vos::ProjectId;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectIssuesRequestId(Uuid);

impl ProjectIssuesRequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl From<Uuid> for ProjectIssuesRequestId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectIssuesPageState {
    Loading {
        request_id: ProjectIssuesRequestId,
    },
    Loaded {
        issues: Vec<ProjectsIssue>,
        total_count: usize,
        offset: usize,
        limit: usize,
    },
    Failed {
        message: String,
    },
}

pub enum ProjectIssuesAction {
    StartLoading {
        request_id: ProjectIssuesRequestId,
        project_id: ProjectId,
        page: NonZeroUsize,
    },
    LoadSucceeded {
        request_id: ProjectIssuesRequestId,
        project_id: ProjectId,
        page: NonZeroUsize,
        result: ProjectIssuesPage,
    },
    LoadFailed {
        request_id: ProjectIssuesRequestId,
        project_id: ProjectId,
        page: NonZeroUsize,
        message: String,
    },
}

pub(super) struct ProjectIssuesStore {
    pages: HashMap<(ProjectId, NonZeroUsize), ProjectIssuesPageState>,
}

impl ProjectIssuesStore {
    pub(super) fn new() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: ProjectIssuesAction) {
        match action {
            ProjectIssuesAction::StartLoading {
                request_id,
                project_id,
                page,
            } => {
                self.pages.insert(
                    (project_id, page),
                    ProjectIssuesPageState::Loading { request_id },
                );
            }
            ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id,
                page,
                result,
            } if self.is_loading_request(request_id, project_id, page) => {
                self.pages.insert(
                    (project_id, page),
                    ProjectIssuesPageState::Loaded {
                        issues: result.issues,
                        total_count: result.total_count,
                        offset: result.offset,
                        limit: result.limit,
                    },
                );
            }
            ProjectIssuesAction::LoadFailed {
                request_id,
                project_id,
                page,
                message,
            } if self.is_loading_request(request_id, project_id, page) => {
                self.pages.insert(
                    (project_id, page),
                    ProjectIssuesPageState::Failed { message },
                );
            }
            _ => {}
        }
    }

    pub(super) fn page_state(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> Option<&ProjectIssuesPageState> {
        self.pages.get(&(project_id, page))
    }

    pub(super) fn issues(
        &self,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> Option<&[ProjectsIssue]> {
        match self.page_state(project_id, page) {
            Some(ProjectIssuesPageState::Loaded { issues, .. }) => Some(issues),
            _ => None,
        }
    }

    fn is_loading_request(
        &self,
        request_id: ProjectIssuesRequestId,
        project_id: ProjectId,
        page: NonZeroUsize,
    ) -> bool {
        matches!(
            self.page_state(project_id, page),
            Some(ProjectIssuesPageState::Loading { request_id: current })
                if *current == request_id
        )
    }
}
