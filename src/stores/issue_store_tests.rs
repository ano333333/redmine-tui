use super::{IssueAction, IssueState, Store};
use crate::test_support::{local_datetime, sample_issue_aggregate};
use crate::vos::IssuePropertyDiff;
use crate::vos::issue_property_diff::{
    IssueDescriptionDiff, IssueDueDateDiff, IssueStartDateDiff, IssueStatusIdDiff,
};
use crate::vos::{CategoryId, IssueId, IssueStatusId, TargetVersionId};

#[test]
fn load_action_is_consumed_through_parent_store() {
    let id = IssueId::new(1);
    let mut store = Store::new();

    store.consume_action(IssueAction::Load { id }.into());

    let (issue, state) = store.get_issue(id).expect("issue should be loaded");
    assert_eq!(issue.issue.id, id);
    assert_eq!(state, &IssueState::Synced);
}

#[test]
fn load_issue_reads_target_version_id_reference() {
    let mut store = Store::new();

    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    let (issue, _) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(issue.target_version_id, Some(TargetVersionId::new(1)));
}

#[test]
fn update_issue_target_version_sets_selected_version() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 2.into() }.into());

    store.consume_action(
        IssueAction::UpdateTargetVersion {
            id: 2.into(),
            target_version_id: Some(TargetVersionId::new(1)),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(2).expect("issue should be loaded");
    assert_eq!(issue.target_version_id, Some(TargetVersionId::new(1)));
    assert_eq!(state, &IssueState::Edited);
}

#[test]
fn update_issue_target_version_can_clear_version() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(
        IssueAction::UpdateTargetVersion {
            id: 1.into(),
            target_version_id: None,
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(issue.target_version_id, None);
    assert_eq!(state, &IssueState::Edited);
}

#[test]
fn load_issue_reads_category_id_reference() {
    let mut store = Store::new();

    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    let (issue, _) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(issue.category_id, Some(CategoryId::new(1)));
}

#[test]
fn update_issue_category_sets_selected_category() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(
        IssueAction::UpdateCategory {
            id: 1.into(),
            category_id: Some(CategoryId::new(2)),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(issue.category_id, Some(CategoryId::new(2)));
    assert_eq!(state, &IssueState::Edited);
}

#[test]
fn update_issue_category_can_clear_category() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(
        IssueAction::UpdateCategory {
            id: 1.into(),
            category_id: None,
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(issue.category_id, None);
    assert_eq!(state, &IssueState::Edited);
}

#[test]
fn update_issue_start_date_updates_issue_and_records_diff() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    let before = store.get_issue(1).unwrap().0.start_date;
    let after = Some(local_datetime("2026-04-30T00:00:00+09:00"));

    store.consume_action(
        IssueAction::UpdateStartDate {
            id: 1.into(),
            start_date: after,
        }
        .into(),
    );

    assert_eq!(store.get_issue(1).unwrap().0.start_date, after);
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)).last(),
        Some(&IssuePropertyDiff::StartDate(IssueStartDateDiff {
            before,
            after
        }))
    );
}

#[test]
fn update_issue_due_date_updates_issue_and_records_diff() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    let before = store.get_issue(1).unwrap().0.due_date;
    let after = Some(local_datetime("2026-05-01T00:00:00+09:00"));

    store.consume_action(
        IssueAction::UpdateDueDate {
            id: 1.into(),
            due_date: after,
        }
        .into(),
    );

    assert_eq!(store.get_issue(1).unwrap().0.due_date, after);
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)).last(),
        Some(&IssuePropertyDiff::DueDate(IssueDueDateDiff {
            before,
            after
        }))
    );
}

#[test]
fn update_issue_description_updates_issue_and_records_diff() {
    let id = IssueId::new(99);
    let mut issue = sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0);
    issue.issue.description = "nested before".to_string();
    let mut store = Store::new();
    store.consume_action(IssueAction::Sync { issue }.into());

    store.consume_action(
        IssueAction::UpdateDescription {
            id,
            body: "nested after".to_string(),
        }
        .into(),
    );

    let (issue, _) = store.get_issue(id).expect("issue should be loaded");
    assert_eq!(issue.issue.description, "nested after");
    assert_eq!(
        store.get_issue_property_diffs(id),
        &[IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "nested before".to_string(),
            after: "nested after".to_string(),
        })]
    );
}

#[test]
fn update_issue_status_updates_issue_and_records_diff() {
    let id = IssueId::new(99);
    let mut issue = sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0);
    issue.issue.status_id = 2.into();
    let mut store = Store::new();
    store.consume_action(IssueAction::Sync { issue }.into());

    store.consume_action(
        IssueAction::UpdateStatus {
            id,
            status_id: 3.into(),
        }
        .into(),
    );

    let (issue, _) = store.get_issue(id).expect("issue should be loaded");
    assert_eq!(issue.issue.status_id, IssueStatusId::new(3));
    assert_eq!(
        store.get_issue_property_diffs(id),
        &[IssuePropertyDiff::StatusId(IssueStatusIdDiff {
            before: 2.into(),
            after: 3.into(),
        })]
    );
}

#[test]
fn start_issue_upload_marks_issue_uploading_and_retains_diffs() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );

    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    let (_, state) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(state, &IssueState::Uploading);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
}

#[test]
fn issue_upload_conflicts_are_retained_while_uploading() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "local body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());
    let server_issue = sample_issue_aggregate(1, "server issue", 1.into(), None, None, None, 0);
    let conflicts = store.get_issue_property_diffs(IssueId::new(1)).to_vec();

    store.consume_action(
        IssueAction::UploadConflictsDetected {
            server_issue: server_issue.clone(),
            conflicts: conflicts.clone(),
        }
        .into(),
    );

    let (actual_issue, actual_conflicts) = store
        .get_issue_upload_conflict(1.into())
        .expect("issue upload conflict should be retained");
    assert_eq!(actual_issue.issue.subject, server_issue.issue.subject);
    assert_eq!(actual_conflicts, conflicts);
    assert_eq!(store.get_issue(1).unwrap().1, &IssueState::Uploading);
}

#[test]
#[should_panic(expected = "cannot update issue while issue 1 is Uploading")]
fn uploading_issue_update_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "late edit".to_string(),
        }
        .into(),
    );
}

macro_rules! missing_issue_update_panics {
    ($($name:ident: $action:expr),+ $(,)?) => {
        $(
            #[test]
            #[should_panic(expected = "cannot update missing issue 99")]
            fn $name() {
                let mut store = Store::new();

                store.consume_action($action.into());
            }
        )+
    };
}

missing_issue_update_panics! {
    missing_issue_update_description_panics: IssueAction::UpdateDescription {
        id: 99.into(),
        body: "body".to_string(),
    },
    missing_issue_update_status_panics: IssueAction::UpdateStatus {
        id: 99.into(),
        status_id: 1.into(),
    },
    missing_issue_update_assigned_to_panics: IssueAction::UpdateAssignedTo {
        id: 99.into(),
        assigned_to_id: None,
    },
    missing_issue_update_target_version_panics: IssueAction::UpdateTargetVersion {
        id: 99.into(),
        target_version_id: None,
    },
    missing_issue_update_category_panics: IssueAction::UpdateCategory {
        id: 99.into(),
        category_id: None,
    },
    missing_issue_update_done_ratio_panics: IssueAction::UpdateDoneRatio {
        id: 99.into(),
        done_ratio: 10,
    },
    missing_issue_update_start_date_panics: IssueAction::UpdateStartDate {
        id: 99.into(),
        start_date: None,
    },
    missing_issue_update_due_date_panics: IssueAction::UpdateDueDate {
        id: 99.into(),
        due_date: None,
    },
}

#[test]
#[should_panic(expected = "cannot start issue upload while issue 1 is Uploading")]
fn uploading_issue_start_upload_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot start issue upload while issue 1 is Synced")]
fn synced_issue_start_upload_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot cancel issue upload while issue 1 is Synced")]
fn synced_issue_cancel_upload_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(IssueAction::CancelUpload { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot cancel issue upload while issue 1 is Edited")]
fn edited_issue_cancel_upload_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );

    store.consume_action(IssueAction::CancelUpload { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot fail issue upload while issue 1 is Synced")]
fn synced_issue_fail_upload_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(
        IssueAction::FailUpload {
            id: 1.into(),
            message: "upload failed".to_string(),
        }
        .into(),
    );
}

#[test]
#[should_panic(expected = "cannot fail issue upload while issue 1 is Edited")]
fn edited_issue_fail_upload_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );

    store.consume_action(
        IssueAction::FailUpload {
            id: 1.into(),
            message: "upload failed".to_string(),
        }
        .into(),
    );
}

#[test]
#[should_panic(expected = "cannot sync issue 1 while it is Synced")]
fn synced_issue_sync_issue_panics() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());

    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(1, "server issue", 1.into(), None, None, None, 0),
        }
        .into(),
    );
}

#[test]
fn cancel_issue_upload_returns_issue_to_edited_and_retains_diffs() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    store.consume_action(IssueAction::CancelUpload { id: 1.into() }.into());

    let (_, state) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(state, &IssueState::Edited);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
}

#[test]
fn fail_issue_upload_returns_issue_to_edited_and_retains_diffs() {
    let mut store = Store::new();
    store.consume_action(IssueAction::Load { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    store.consume_action(
        IssueAction::FailUpload {
            id: 1.into(),
            message: "upload failed".to_string(),
        }
        .into(),
    );

    let (_, state) = store.get_issue(1).expect("issue should be loaded");
    assert_eq!(state, &IssueState::Edited);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
}

#[test]
fn sync_issue_replaces_issue_clears_diffs_and_marks_synced() {
    let mut store = Store::new();
    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(
                9,
                "server issue before edit",
                1.into(),
                None,
                None,
                None,
                0,
            ),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 9.into(),
            body: "local edit".to_string(),
        }
        .into(),
    );

    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(
                9,
                "server issue after upload",
                1.into(),
                None,
                None,
                None,
                0,
            ),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(9).expect("issue should be synced");
    assert_eq!(issue.issue.subject, "server issue after upload");
    assert_eq!(issue.issue.description, "body");
    assert_eq!(state, &IssueState::Synced);
    assert!(store.get_issue_property_diffs(IssueId::new(9)).is_empty());
}

#[test]
fn upload_success_sync_issue_replaces_issue_clears_diffs_and_marks_synced() {
    let mut store = Store::new();
    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(
                9,
                "server issue before edit",
                1.into(),
                None,
                None,
                None,
                0,
            ),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 9.into(),
            body: "local edit".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 9.into() }.into());

    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(
                9,
                "server issue after upload",
                1.into(),
                None,
                None,
                None,
                0,
            ),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(9).expect("issue should be synced");
    assert_eq!(issue.issue.subject, "server issue after upload");
    assert_eq!(issue.issue.description, "body");
    assert_eq!(state, &IssueState::Synced);
    assert!(store.get_issue_property_diffs(IssueId::new(9)).is_empty());
}

#[test]
fn unregistered_issue_can_start_fetching_without_an_issue_body() {
    let id = IssueId::new(99);
    let mut store = Store::new();

    store.consume_action(IssueAction::StartFetching { id }.into());

    assert_eq!(store.get_issue_state(id), Some(&IssueState::Fetching));
    assert!(store.get_issue(id).is_none());
}

#[test]
fn failed_issue_can_restart_fetching_and_clears_the_error() {
    let id = IssueId::new(99);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id }.into());
    store.consume_action(
        IssueAction::FetchFailed {
            id,
            message: "network error".to_string(),
        }
        .into(),
    );

    store.consume_action(IssueAction::StartFetching { id }.into());

    assert_eq!(store.get_issue_state(id), Some(&IssueState::Fetching));
    assert!(store.get_issue(id).is_none());
}

macro_rules! start_fetching_outside_startable_states_panics {
    ($($name:ident: $expected:literal => [$($setup:expr),* $(,)?]),+ $(,)?) => {
        $(
            #[test]
            #[should_panic(expected = $expected)]
            fn $name() {
                let id = IssueId::new(99);
                let mut store = Store::new();
                $(store.consume_action($setup.into());)*

                store.consume_action(IssueAction::StartFetching { id }.into());
            }
        )+
    };
}

start_fetching_outside_startable_states_panics! {
    start_fetching_while_fetching_panics:
        "cannot start fetching issue 99 while it is Some(Fetching)" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
        ],
    start_fetching_while_synced_panics:
        "cannot start fetching issue 99 while it is Some(Synced)" => [IssueAction::Sync {
            issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
        }],
    start_fetching_while_edited_panics:
        "cannot start fetching issue 99 while it is Some(Edited)" => [
            IssueAction::Sync {
                issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
            },
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
        ],
    start_fetching_while_uploading_panics:
        "cannot start fetching issue 99 while it is Some(Uploading)" => [
            IssueAction::Sync {
                issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
            },
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
            IssueAction::StartUpload { id: IssueId::new(99) },
        ],
}

#[test]
fn matching_fetch_success_registers_the_issue_as_synced() {
    let id = IssueId::new(99);
    let issue = sample_issue_aggregate(99, "fetched", 1.into(), None, None, None, 0);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id }.into());

    store.consume_action(IssueAction::FetchSucceeded { id, issue }.into());

    let (issue, state) = store.get_issue(id).expect("issue should be fetched");
    assert_eq!(issue.issue.subject, "fetched");
    assert_eq!(state, &IssueState::Synced);
    assert!(store.get_issue_property_diffs(id).is_empty());
    assert!(store.get_issue_upload_conflict(id).is_none());
}

#[test]
#[should_panic(expected = "fetch succeeded with mismatched issue id: requested 99, got 100")]
fn mismatched_fetch_success_panics() {
    let requested_id = IssueId::new(99);
    let response_issue = sample_issue_aggregate(100, "wrong", 1.into(), None, None, None, 0);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id: requested_id }.into());

    store.consume_action(
        IssueAction::FetchSucceeded {
            id: requested_id,
            issue: response_issue,
        }
        .into(),
    );
}

#[test]
fn fetch_failure_retains_message_without_an_issue_body() {
    let id = IssueId::new(99);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id }.into());

    store.consume_action(
        IssueAction::FetchFailed {
            id,
            message: "network error".to_string(),
        }
        .into(),
    );

    assert_eq!(
        store.get_issue_state(id),
        Some(&IssueState::FetchFailed {
            message: "network error".to_string()
        })
    );
    assert!(store.get_issue(id).is_none());
}

macro_rules! fetch_failure_outside_fetching_panics {
    ($($name:ident: $expected:literal => [$($setup:expr),* $(,)?]),+ $(,)?) => {
        $(
            #[test]
            #[should_panic(expected = $expected)]
            fn $name() {
                let id = IssueId::new(99);
                let mut store = Store::new();
                $(store.consume_action($setup.into());)*

                store.consume_action(
                    IssueAction::FetchFailed {
                        id,
                        message: "late error".to_string(),
                    }
                    .into(),
                );
            }
        )+
    };
}

fetch_failure_outside_fetching_panics! {
    fetch_failure_without_state_panics:
        "fetch failed for issue 99 without an issue state" => [],
    fetch_failure_while_synced_panics:
        "fetch failed while issue 99 is Synced" => [IssueAction::Sync {
            issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
        }],
    fetch_failure_after_fetch_failure_panics:
        "fetch failed while issue 99 is FetchFailed" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            IssueAction::FetchFailed {
                id: IssueId::new(99),
                message: "original error".to_string(),
            },
        ],
    fetch_failure_while_edited_panics:
        "fetch failed while issue 99 is Edited" => [
            IssueAction::Sync {
                issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
            },
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
        ],
    fetch_failure_while_uploading_panics:
        "fetch failed while issue 99 is Uploading" => [
            IssueAction::Sync {
                issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
            },
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
            IssueAction::StartUpload { id: IssueId::new(99) },
        ],
}

macro_rules! fetch_success_outside_fetching_panics {
    ($($name:ident: $expected:literal => [$($setup:expr),* $(,)?]),+ $(,)?) => {
        $(
            #[test]
            #[should_panic(expected = $expected)]
            fn $name() {
                let id = IssueId::new(99);
                let mut store = Store::new();
                $(store.consume_action($setup.into());)*

                store.consume_action(
                    IssueAction::FetchSucceeded {
                        id,
                        issue: sample_issue_aggregate(99, "late", 1.into(), None, None, None, 0),
                    }
                    .into(),
                );
            }
        )+
    };
}

fetch_success_outside_fetching_panics! {
    fetch_success_without_state_panics:
        "fetch succeeded for issue 99 without an issue state" => [],
    fetch_success_while_synced_panics:
        "fetch succeeded while issue 99 is Synced" => [IssueAction::Sync {
            issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
        }],
    fetch_success_after_fetch_failure_panics:
        "fetch succeeded while issue 99 is FetchFailed" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            IssueAction::FetchFailed {
                id: IssueId::new(99),
                message: "original error".to_string(),
            },
        ],
    fetch_success_while_edited_panics:
        "fetch succeeded while issue 99 is Edited" => [
            IssueAction::Sync {
                issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
            },
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
        ],
    fetch_success_while_uploading_panics:
        "fetch succeeded while issue 99 is Uploading" => [
            IssueAction::Sync {
                issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
            },
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
            IssueAction::StartUpload { id: IssueId::new(99) },
        ],
}

#[test]
fn fixture_load_does_not_add_a_body_to_fetching_or_failed_issues() {
    for fail_fetch in [false, true] {
        let id = IssueId::new(1);
        let mut store = Store::new();
        store.consume_action(IssueAction::StartFetching { id }.into());
        if fail_fetch {
            store.consume_action(
                IssueAction::FetchFailed {
                    id,
                    message: "failed".to_string(),
                }
                .into(),
            );
        }

        store.consume_action(IssueAction::Load { id }.into());

        assert!(store.get_issue(id).is_none());
        if fail_fetch {
            assert_eq!(
                store.get_issue_state(id),
                Some(&IssueState::FetchFailed {
                    message: "failed".to_string(),
                })
            );
        } else {
            assert_eq!(store.get_issue_state(id), Some(&IssueState::Fetching));
        }
    }
}

#[test]
#[should_panic(expected = "cannot sync issue 99 while it is Fetching")]
fn fetching_issue_sync_panics() {
    let id = IssueId::new(99);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id }.into());

    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(99, "sync", 1.into(), None, None, None, 0),
        }
        .into(),
    );
}

#[test]
#[should_panic(expected = "cannot sync issue 99 while it is FetchFailed")]
fn fetch_failed_issue_sync_panics() {
    let id = IssueId::new(99);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id }.into());
    store.consume_action(
        IssueAction::FetchFailed {
            id,
            message: "failed".to_string(),
        }
        .into(),
    );

    store.consume_action(
        IssueAction::Sync {
            issue: sample_issue_aggregate(99, "sync", 1.into(), None, None, None, 0),
        }
        .into(),
    );
}
