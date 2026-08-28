use std::num::NonZeroUsize;

use super::project_issues_store::ProjectIssuesStore;
use super::{ProjectIssuesAction, ProjectIssuesPageState, ProjectIssuesRequestId};
use crate::entities::{ProjectIssuesPage, ProjectsIssue};
use crate::vos::{EntityIdValue, IssueId, IssueStatusId, ProjectId};
use uuid::Uuid;

fn request_id(value: u128) -> ProjectIssuesRequestId {
    Uuid::from_u128(value).into()
}

fn page(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test pages are one-based")
}

fn issue(issue_id: u16, project_id: u16) -> ProjectsIssue {
    ProjectsIssue {
        issue_id: IssueId::new(issue_id),
        project_id: ProjectId::new(project_id),
        subject: format!("issue {issue_id}"),
        description: String::new(),
        status_id: IssueStatusId::new(1),
    }
}

fn result(issues: Vec<ProjectsIssue>, total_count: usize, offset: usize) -> ProjectIssuesPage {
    ProjectIssuesPage {
        issues,
        total_count,
        offset,
        limit: 50,
    }
}

fn consume(store: &mut ProjectIssuesStore, action: ProjectIssuesAction) {
    store.consume_action(action);
}

fn start(
    store: &mut ProjectIssuesStore,
    request_id: ProjectIssuesRequestId,
    project_id: ProjectId,
    project_page: NonZeroUsize,
) {
    consume(
        store,
        ProjectIssuesAction::StartLoading {
            request_id,
            project_id,
            page: project_page,
        },
    );
}

fn succeed(
    store: &mut ProjectIssuesStore,
    request_id: ProjectIssuesRequestId,
    project_id: ProjectId,
    project_page: NonZeroUsize,
    issue_id: u16,
) {
    consume(
        store,
        ProjectIssuesAction::LoadSucceeded {
            request_id,
            project_id,
            page: project_page,
            result: result(vec![issue(issue_id, project_id.get())], 101, 0),
        },
    );
}

#[test]
fn start_loading_replaces_missing_loading_loaded_and_failed_exact_keys() {
    let project_id = ProjectId::new(10);
    let project_page = page(1);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, request_id(1), project_id, project_page);
    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: request_id(1),
        })
    );

    start(&mut store, request_id(2), project_id, project_page);
    succeed(&mut store, request_id(2), project_id, project_page, 42);
    start(&mut store, request_id(3), project_id, project_page);
    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: request_id(3),
        })
    );

    consume(
        &mut store,
        ProjectIssuesAction::LoadFailed {
            request_id: request_id(3),
            project_id,
            page: project_page,
            message: "offline".to_string(),
        },
    );
    start(&mut store, request_id(4), project_id, project_page);
    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: request_id(4),
        })
    );
}

#[test]
fn same_exact_key_accepts_only_the_latest_started_request_completion() {
    let project_id = ProjectId::new(10);
    let project_page = page(1);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, request_id(1), project_id, project_page);
    start(&mut store, request_id(3), project_id, project_page);
    succeed(&mut store, request_id(1), project_id, project_page, 41);
    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: request_id(3),
        })
    );

    consume(
        &mut store,
        ProjectIssuesAction::LoadFailed {
            request_id: request_id(1),
            project_id,
            page: project_page,
            message: "stale".to_string(),
        },
    );
    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: request_id(3),
        })
    );

    succeed(&mut store, request_id(3), project_id, project_page, 43);
    assert_eq!(
        store.issues(project_id, project_page).unwrap()[0].subject,
        "issue 43"
    );
}

#[test]
fn different_exact_keys_complete_independently() {
    let first_project = ProjectId::new(10);
    let second_project = ProjectId::new(20);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, request_id(1), first_project, page(1));
    start(&mut store, request_id(2), second_project, page(2));
    succeed(&mut store, request_id(2), second_project, page(2), 82);
    succeed(&mut store, request_id(1), first_project, page(1), 41);

    assert_eq!(
        store.issues(first_project, page(1)).unwrap()[0].issue_id,
        IssueId::new(41)
    );
    assert_eq!(
        store.issues(second_project, page(2)).unwrap()[0].issue_id,
        IssueId::new(82)
    );
}

#[test]
fn different_pages_of_the_same_project_complete_independently() {
    let project_id = ProjectId::new(10);
    let first_page = page(1);
    let second_page = page(2);
    let first_request = request_id(1);
    let second_request = request_id(2);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, first_request, project_id, first_page);
    start(&mut store, second_request, project_id, second_page);
    assert_eq!(
        store.page_state(project_id, first_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: first_request,
        })
    );
    assert_eq!(
        store.page_state(project_id, second_page),
        Some(&ProjectIssuesPageState::Loading {
            request_id: second_request,
        })
    );

    succeed(&mut store, second_request, project_id, second_page, 82);
    succeed(&mut store, first_request, project_id, first_page, 41);

    assert_eq!(
        store.issues(project_id, first_page).unwrap()[0].issue_id,
        IssueId::new(41)
    );
    assert_eq!(
        store.issues(project_id, second_page).unwrap()[0].issue_id,
        IssueId::new(82)
    );
}

#[test]
fn matching_load_failure_transitions_the_exact_key_to_failed_with_its_message() {
    let project_id = ProjectId::new(10);
    let project_page = page(1);
    let request_id = request_id(1);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, request_id, project_id, project_page);
    consume(
        &mut store,
        ProjectIssuesAction::LoadFailed {
            request_id,
            project_id,
            page: project_page,
            message: "offline".to_string(),
        },
    );

    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Failed {
            message: "offline".to_string(),
        })
    );
}

#[test]
fn completion_for_a_missing_or_different_exact_key_is_ignored() {
    let project_id = ProjectId::new(10);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, request_id(1), project_id, page(1));
    succeed(&mut store, request_id(1), project_id, page(2), 42);
    consume(
        &mut store,
        ProjectIssuesAction::LoadFailed {
            request_id: request_id(1),
            project_id: ProjectId::new(20),
            page: page(1),
            message: "wrong key".to_string(),
        },
    );

    assert_eq!(store.page_state(project_id, page(2)), None);
    assert_eq!(store.page_state(ProjectId::new(20), page(1)), None);
    assert_eq!(
        store.page_state(project_id, page(1)),
        Some(&ProjectIssuesPageState::Loading {
            request_id: request_id(1),
        })
    );
}

#[test]
fn loaded_page_preserves_metadata_and_empty_issues() {
    let project_id = ProjectId::new(10);
    let project_page = page(3);
    let request_id = request_id(1);
    let mut store = ProjectIssuesStore::new();

    start(&mut store, request_id, project_id, project_page);
    consume(
        &mut store,
        ProjectIssuesAction::LoadSucceeded {
            request_id,
            project_id,
            page: project_page,
            result: result(Vec::new(), 100, 100),
        },
    );

    assert_eq!(
        store.page_state(project_id, project_page),
        Some(&ProjectIssuesPageState::Loaded {
            issues: Vec::new(),
            total_count: 100,
            offset: 100,
            limit: 50,
        })
    );
    assert_eq!(store.issues(project_id, project_page), Some([].as_slice()));
}
