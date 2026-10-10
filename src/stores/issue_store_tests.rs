use super::{Action, IssueAction, IssueFetchState, IssueState, Store};
use crate::entities::IssueAggregate;
use crate::test_support::{local_datetime, sample_issue_aggregate};
use crate::vos::IssuePropertyDiff;
use crate::vos::id::EntityIdValue;
use crate::vos::issue_property_diff::{
    IssueCategoryIdDiff, IssueDescriptionDiff, IssueDueDateDiff, IssueEstimatedHoursDiff,
    IssuePriorityIdDiff, IssueProjectIdDiff, IssueStartDateDiff, IssueStatusIdDiff,
    IssueTargetVersionIdDiff, IssueTrackerIdDiff,
};
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId,
};

const ISSUE_START_DATE: &str = "2025-12-09T00:00:00+09:00";
const ISSUE_DUE_DATE: &str = "2025-12-19T00:00:00+09:00";

fn sync_issue(store: &mut Store, id: IssueId) {
    let mut issue = sample_issue_aggregate(
        id.get(),
        "issue",
        3.into(),
        None,
        Some(ISSUE_START_DATE),
        Some(ISSUE_DUE_DATE),
        0,
    );
    issue.issue.project_id = ProjectId::new(1);
    issue.tracker_id = TrackerId::new(1);
    issue.priority_id = PriorityId::new(1);
    issue.target_version_id = Some(TargetVersionId::new(1));
    issue.category_id = Some(CategoryId::new(1));
    issue.estimated_hours = None;
    crate::test_support::load_issue(store, issue);
}

#[test]
fn get_issues_lists_only_loaded_issues_and_excludes_unfetched_states() {
    let mut store = Store::new();

    sync_issue(&mut store, 1.into());
    store.consume_action(
        IssueAction::StartFetching {
            id: IssueId::new(99),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::FetchFailed {
            id: IssueId::new(99),
            message: "network error".to_string(),
        }
        .into(),
    );

    let issue_ids: Vec<IssueId> = store.get_issues().map(|(issue, _)| issue.id()).collect();
    assert_eq!(issue_ids, vec![IssueId::new(1)]);
}

#[test]
fn update_issue_target_version_sets_selected_version() {
    let mut store = Store::new();
    sync_issue(&mut store, 2.into());

    store.consume_action(
        IssueAction::UpdateTargetVersion {
            id: 2.into(),
            target_version_id: Some(TargetVersionId::new(2)),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(2);
    assert_eq!(issue.target_version_id(), Some(TargetVersionId::new(2)));
    assert_eq!(state, IssueState::Edited);
}

#[test]
fn update_issue_target_version_can_clear_version() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdateTargetVersion {
            id: 1.into(),
            target_version_id: None,
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.target_version_id(), None);
    assert_eq!(state, IssueState::Edited);
}

#[test]
fn update_issue_category_sets_selected_category() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdateCategory {
            id: 1.into(),
            category_id: Some(CategoryId::new(2)),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.category_id(), Some(CategoryId::new(2)));
    assert_eq!(state, IssueState::Edited);
}

#[test]
fn update_issue_category_can_clear_category() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdateCategory {
            id: 1.into(),
            category_id: None,
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.category_id(), None);
    assert_eq!(state, IssueState::Edited);
}

#[test]
fn update_issue_tracker_updates_issue_and_records_diff() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdateTracker {
            id: 1.into(),
            tracker_id: TrackerId::new(2),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.tracker_id(), TrackerId::new(2));
    assert_eq!(state, IssueState::Edited);
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)).last(),
        Some(&IssuePropertyDiff::TrackerId(IssueTrackerIdDiff {
            before: TrackerId::new(1),
            after: TrackerId::new(2),
        }))
    );
}

#[test]
fn update_issue_project_clears_target_version_and_category_and_records_diffs() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdateProject {
            id: 1.into(),
            project_id: ProjectId::new(2),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.project_id(), ProjectId::new(2));
    assert_eq!(issue.target_version_id(), None);
    assert_eq!(issue.category_id(), None);
    assert_eq!(state, IssueState::Edited);
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)),
        &[
            IssuePropertyDiff::ProjectId(IssueProjectIdDiff {
                before: ProjectId::new(1),
                after: ProjectId::new(2),
            }),
            IssuePropertyDiff::TargetVersionId(IssueTargetVersionIdDiff {
                before: Some(TargetVersionId::new(1)),
                after: None,
            }),
            IssuePropertyDiff::CategoryId(IssueCategoryIdDiff {
                before: Some(CategoryId::new(1)),
                after: None,
            }),
        ]
    );
}

#[test]
fn update_issue_project_records_only_project_diff_when_related_values_are_unset() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
    store.consume_action(
        IssueAction::UpdateTargetVersion {
            id: 1.into(),
            target_version_id: None,
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UpdateCategory {
            id: 1.into(),
            category_id: None,
        }
        .into(),
    );

    store.consume_action(
        IssueAction::UpdateProject {
            id: 1.into(),
            project_id: ProjectId::new(2),
        }
        .into(),
    );

    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)).last(),
        Some(&IssuePropertyDiff::ProjectId(IssueProjectIdDiff {
            before: ProjectId::new(1),
            after: ProjectId::new(2),
        }))
    );
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 3);
}

#[test]
fn update_issue_priority_updates_issue_and_records_diff() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdatePriority {
            id: 1.into(),
            priority_id: PriorityId::new(3),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.priority_id(), PriorityId::new(3));
    assert_eq!(state, IssueState::Edited);
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)).last(),
        Some(&IssuePropertyDiff::PriorityId(IssuePriorityIdDiff {
            before: PriorityId::new(1),
            after: PriorityId::new(3),
        }))
    );
}

#[test]
fn update_issue_estimated_hours_updates_issue_and_records_diff() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(
        IssueAction::UpdateEstimatedHours {
            id: 1.into(),
            estimated_hours: Some(2.5),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.estimated_hours(), Some(2.5));
    assert_eq!(state, IssueState::Edited);
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)).last(),
        Some(&IssuePropertyDiff::EstimatedHours(
            IssueEstimatedHoursDiff {
                before: None,
                after: Some(2.5),
            }
        ))
    );
}

#[test]
fn update_issue_start_date_updates_issue_and_records_diff() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
    let before = Some(local_datetime(ISSUE_START_DATE));
    let after = Some(local_datetime("2026-04-30T00:00:00+09:00"));

    store.consume_action(
        IssueAction::UpdateStartDate {
            id: 1.into(),
            start_date: after,
        }
        .into(),
    );

    assert_eq!(store.get_issue(1).0.start_date(), after);
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
    sync_issue(&mut store, 1.into());
    let before = Some(local_datetime(ISSUE_DUE_DATE));
    let after = Some(local_datetime("2026-05-01T00:00:00+09:00"));

    store.consume_action(
        IssueAction::UpdateDueDate {
            id: 1.into(),
            due_date: after,
        }
        .into(),
    );

    assert_eq!(store.get_issue(1).0.due_date(), after);
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
    crate::test_support::load_issue(&mut store, issue);

    store.consume_action(
        IssueAction::UpdateDescription {
            id,
            body: "nested after".to_string(),
        }
        .into(),
    );

    let (issue, _) = store.get_issue(id);
    assert_eq!(issue.description(), "nested after");
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
    crate::test_support::load_issue(&mut store, issue);

    store.consume_action(
        IssueAction::UpdateStatus {
            id,
            status_id: 3.into(),
        }
        .into(),
    );

    let (issue, _) = store.get_issue(id);
    assert_eq!(issue.status_id(), IssueStatusId::new(3));
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
    sync_issue(&mut store, 1.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );

    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    let (_, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Uploading);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
}

#[test]
fn uploading_issue_update_panics_and_retains_the_entry() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.consume_action(
            IssueAction::UpdateDescription {
                id: 1.into(),
                body: "late edit".to_string(),
            }
            .into(),
        );
    }))
    .is_err();

    assert!(panicked);
    assert_eq!(store.get_issue(1).1, IssueState::Uploading);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
}

#[test]
fn issue_upload_conflicts_are_retained_while_uploading() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
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
            children: vec![],
        }
        .into(),
    );

    let (actual_issue, actual_conflicts) = store
        .try_get_issue_upload_conflict(1.into())
        .expect("issue upload conflict should be retained");
    assert_eq!(actual_issue.issue.subject, server_issue.issue.subject);
    assert_eq!(actual_conflicts, conflicts);
    assert_eq!(store.get_issue(1).1, IssueState::Uploading);
}

/// Issue 1の説明を"local body"へ編集して保存を始めた状態にする。
fn start_description_upload(store: &mut Store) {
    sync_issue(store, 1.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "local body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());
}

fn description_diff(before: &str, after: &str) -> IssuePropertyDiff {
    IssuePropertyDiff::Description(IssueDescriptionDiff {
        before: before.to_string(),
        after: after.to_string(),
    })
}

#[test]
fn upload_preflight_replaces_the_base_and_the_diffs_while_uploading() {
    let mut store = Store::new();
    start_description_upload(&mut store);
    let mut server_issue =
        sample_issue_aggregate(1, "server subject", 1.into(), None, None, None, 0);
    server_issue.issue.description = "server body".to_string();

    store.consume_action(
        IssueAction::UploadPreflightSucceeded {
            server_issue,
            children: vec![crate::entities::IssueChild {
                id: IssueId::new(2),
                tracker_id: TrackerId::new(1),
                subject: "child".to_string(),
                children: vec![],
            }],
            diffs: vec![description_diff("server body", "resolved body")],
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Uploading);
    assert_eq!(issue.subject(), "server subject");
    assert_eq!(issue.description(), "resolved body");
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)),
        [description_diff("server body", "resolved body")]
    );
    assert_eq!(store.get_issue_children(1).len(), 1);
}

#[test]
fn upload_put_applies_the_sent_diffs_to_the_base_and_keeps_uploading() {
    let mut store = Store::new();
    start_description_upload(&mut store);
    let diffs = store.get_issue_property_diffs(IssueId::new(1)).to_vec();

    store.consume_action(
        IssueAction::UploadPutSucceeded {
            id: 1.into(),
            diffs,
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Uploading);
    assert_eq!(issue.description(), "local body");
    assert!(store.get_issue_property_diffs(IssueId::new(1)).is_empty());
}

#[test]
fn upload_confirm_failure_keeps_the_put_result_as_synced() {
    let mut store = Store::new();
    start_description_upload(&mut store);
    let diffs = store.get_issue_property_diffs(IssueId::new(1)).to_vec();
    store.consume_action(
        IssueAction::UploadPutSucceeded {
            id: 1.into(),
            diffs,
        }
        .into(),
    );

    store.consume_action(IssueAction::UploadConfirmFailed { id: 1.into() }.into());

    let (issue, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Synced);
    assert_eq!(issue.description(), "local body");
}

#[test]
#[should_panic(
    expected = "cannot finish issue upload without confirmation while issue 1 is Uploading or has unsent diffs"
)]
fn upload_confirm_failure_before_the_put_panics() {
    let mut store = Store::new();
    start_description_upload(&mut store);

    store.consume_action(IssueAction::UploadConfirmFailed { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot update issue while issue 1 is Uploading")]
fn uploading_issue_update_panics() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
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
    missing_issue_update_tracker_panics: IssueAction::UpdateTracker {
        id: 99.into(),
        tracker_id: 1.into(),
    },
    missing_issue_update_project_panics: IssueAction::UpdateProject {
        id: 99.into(),
        project_id: 1.into(),
    },
    missing_issue_update_priority_panics: IssueAction::UpdatePriority {
        id: 99.into(),
        priority_id: 1.into(),
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
    missing_issue_update_estimated_hours_panics: IssueAction::UpdateEstimatedHours {
        id: 99.into(),
        estimated_hours: None,
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
    sync_issue(&mut store, 1.into());
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
    sync_issue(&mut store, 1.into());

    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot cancel issue upload while issue 1 is Synced")]
fn synced_issue_cancel_upload_panics() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());

    store.consume_action(IssueAction::CancelUpload { id: 1.into() }.into());
}

#[test]
#[should_panic(expected = "cannot cancel issue upload while issue 1 is Edited")]
fn edited_issue_cancel_upload_panics() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
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
    sync_issue(&mut store, 1.into());

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
    sync_issue(&mut store, 1.into());
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
fn cancel_issue_upload_returns_issue_to_edited_and_retains_diffs() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    store.consume_action(IssueAction::CancelUpload { id: 1.into() }.into());

    let (_, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Edited);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
}

#[test]
fn fail_issue_upload_returns_issue_to_edited_and_retains_diffs_and_message() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());
    store.consume_action(
        IssueAction::UploadConflictsDetected {
            server_issue: sample_issue_aggregate(1, "server issue", 1.into(), None, None, None, 0),
            conflicts: store.get_issue_property_diffs(1).to_vec(),
            children: vec![],
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

    let (_, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Edited);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
    assert!(store.try_get_issue_upload_conflict(1.into()).is_none());
    assert_eq!(
        store.try_get_issue_upload_failure(1.into()),
        Some("upload failed")
    );

    store.consume_action(IssueAction::StartUpload { id: 1.into() }.into());

    assert_eq!(store.try_get_issue_upload_failure(1.into()), None);
}

#[test]
fn update_after_failed_upload_appends_diff_and_retains_failure() {
    let mut store = Store::new();
    sync_issue(&mut store, 1.into());
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
            message: "temporary failure".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UpdateStatus {
            id: 1.into(),
            status_id: 2.into(),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(issue.status_id(), IssueStatusId::new(2));
    assert_eq!(state, IssueState::Edited);
    let diffs = store.get_issue_property_diffs(IssueId::new(1));
    assert_eq!(diffs.len(), 2);
    assert_eq!(
        diffs[0],
        IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "body".to_string(),
            after: "edited body".to_string(),
        })
    );
    assert_eq!(
        diffs[1],
        IssuePropertyDiff::StatusId(IssueStatusIdDiff {
            before: 3.into(),
            after: 2.into(),
        })
    );
    assert_eq!(
        store.try_get_issue_upload_failure(1.into()),
        Some("temporary failure")
    );
}

#[test]
fn unregistered_issue_can_start_fetching_without_an_issue_body() {
    let id = IssueId::new(99);
    let mut store = Store::new();

    store.consume_action(IssueAction::StartFetching { id }.into());

    assert_eq!(
        store.try_get_issue_fetch_state(id),
        Some(IssueFetchState::Fetching)
    );
    assert!(store.try_get_issue_state(id).is_none());
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

    assert_eq!(
        store.try_get_issue_fetch_state(id),
        Some(IssueFetchState::Fetching)
    );
    assert!(store.try_get_issue_state(id).is_none());
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
        "cannot start fetching issue 99 while it is Fetching" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
        ],
    start_fetching_while_synced_panics:
        "cannot start fetching issue 99 while it is Synced" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
        ],
    start_fetching_while_edited_panics:
        "cannot start fetching issue 99 while it is Edited" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
        ],
    start_fetching_while_uploading_panics:
        "cannot start fetching issue 99 while it is Uploading" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
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

    store.consume_action(Action::IssueFetchSucceeded {
        id,
        issue,
        children: vec![],
    });

    let (issue, state) = store.get_issue(id);
    assert_eq!(issue.subject(), "fetched");
    assert_eq!(state, IssueState::Synced);
    assert!(store.get_issue_property_diffs(id).is_empty());
    assert!(store.try_get_issue_upload_conflict(id).is_none());
}

#[test]
#[should_panic(expected = "fetch succeeded with mismatched issue id: requested 99, got 100")]
fn mismatched_fetch_success_panics() {
    let requested_id = IssueId::new(99);
    let response_issue = sample_issue_aggregate(100, "wrong", 1.into(), None, None, None, 0);
    let mut store = Store::new();
    store.consume_action(IssueAction::StartFetching { id: requested_id }.into());

    store.consume_action(Action::IssueFetchSucceeded {
        id: requested_id,
        issue: response_issue,
        children: vec![],
    });
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
        store.try_get_issue_fetch_state(id),
        Some(IssueFetchState::FetchFailed {
            message: "network error".to_string()
        })
    );
    assert!(store.try_get_issue_state(id).is_none());
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
        "fetch failed while issue 99 is Synced" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
        ],
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
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
        ],
    fetch_failure_while_uploading_panics:
        "fetch failed while issue 99 is Uploading" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
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

                store.consume_action(Action::IssueFetchSucceeded {
                    id,
                    issue: sample_issue_aggregate(99, "late", 1.into(), None, None, None, 0),
                    children: vec![],
                });
            }
        )+
    };
}

fetch_success_outside_fetching_panics! {
    fetch_success_without_state_panics:
        "fetch succeeded for issue 99 without an issue state" => [],
    fetch_success_while_synced_panics:
        "fetch succeeded while issue 99 is Synced" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
        ],
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
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
        ],
    fetch_success_while_uploading_panics:
        "fetch succeeded while issue 99 is Uploading" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            fetched_issue_99(),
            IssueAction::UpdateDescription {
                id: IssueId::new(99),
                body: "local edit".to_string(),
            },
            IssueAction::StartUpload { id: IssueId::new(99) },
        ],
}

#[test]
fn state_getters_split_loaded_and_fetch_states() {
    let id = IssueId::new(99);
    let edit = || IssueAction::UpdateDescription {
        id,
        body: "local edit".to_string(),
    };
    let start = || Action::from(IssueAction::StartFetching { id });
    let cases: Vec<(Vec<Action>, Option<IssueState>, Option<IssueFetchState>)> = vec![
        (vec![], None, None),
        (vec![start()], None, Some(IssueFetchState::Fetching)),
        (
            vec![
                start(),
                IssueAction::FetchFailed {
                    id,
                    message: "failed".to_string(),
                }
                .into(),
            ],
            None,
            Some(IssueFetchState::FetchFailed {
                message: "failed".to_string(),
            }),
        ),
        (
            vec![start(), fetched_issue_99()],
            Some(IssueState::Synced),
            None,
        ),
        (
            vec![start(), fetched_issue_99(), edit().into()],
            Some(IssueState::Edited),
            None,
        ),
        (
            vec![
                start(),
                fetched_issue_99(),
                edit().into(),
                IssueAction::StartUpload { id }.into(),
            ],
            Some(IssueState::Uploading),
            None,
        ),
    ];

    for (setup, expected_state, expected_fetch_state) in cases {
        let mut store = Store::new();
        for action in setup {
            store.consume_action(action);
        }

        assert_eq!(store.try_get_issue_state(id), expected_state);
        assert_eq!(store.try_get_issue_fetch_state(id), expected_fetch_state);
    }
}

macro_rules! get_issue_without_a_loaded_body_panics {
    ($($name:ident: $expected:literal => [$($setup:expr),* $(,)?]),+ $(,)?) => {
        $(
            #[test]
            #[should_panic(expected = $expected)]
            fn $name() {
                let id = IssueId::new(99);
                let mut store = Store::new();
                $(store.consume_action($setup.into());)*

                store.get_issue(id);
            }
        )+
    };
}

get_issue_without_a_loaded_body_panics! {
    get_issue_while_unregistered_panics:
        "cannot get issue 99 while it is Unregistered" => [],
    get_issue_while_fetching_panics:
        "cannot get issue 99 while it is Fetching" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
        ],
    get_issue_while_fetch_failed_panics:
        "cannot get issue 99 while it is FetchFailed" => [
            IssueAction::StartFetching { id: IssueId::new(99) },
            IssueAction::FetchFailed {
                id: IssueId::new(99),
                message: "failed".to_string(),
            },
        ],
}

#[test]
fn editing_a_property_back_to_the_fetched_value_returns_to_synced() {
    let mut store = Store::new();
    crate::test_support::load_issue(&mut store, issue_with_description(1, "fetched"));

    for body in ["edited", "fetched"] {
        store.consume_action(
            IssueAction::UpdateDescription {
                id: 1.into(),
                body: body.to_string(),
            }
            .into(),
        );
    }

    let (issue, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Synced);
    assert_eq!(issue.description(), "fetched");
    assert!(store.get_issue_property_diffs(IssueId::new(1)).is_empty());
}

#[test]
fn edits_keep_the_fetched_value_as_the_first_before_and_record_each_step() {
    let mut store = Store::new();
    crate::test_support::load_issue(&mut store, issue_with_description(1, "fetched"));

    for body in ["first", "second"] {
        store.consume_action(
            IssueAction::UpdateDescription {
                id: 1.into(),
                body: body.to_string(),
            }
            .into(),
        );
    }

    let (issue, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Edited);
    assert_eq!(issue.description(), "second");
    assert_eq!(
        store.get_issue_property_diffs(IssueId::new(1)),
        &[
            IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "fetched".to_string(),
                after: "first".to_string(),
            }),
            IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "first".to_string(),
                after: "second".to_string(),
            }),
        ]
    );
}

#[test]
fn reverting_one_property_keeps_other_edits_and_the_edit_history() {
    let mut store = Store::new();
    crate::test_support::load_issue(&mut store, issue_with_description(1, "fetched"));

    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "edited".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UpdateDoneRatio {
            id: 1.into(),
            done_ratio: 50,
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UpdateDescription {
            id: 1.into(),
            body: "fetched".to_string(),
        }
        .into(),
    );

    let (issue, state) = store.get_issue(1);
    assert_eq!(state, IssueState::Edited);
    assert_eq!(issue.description(), "fetched");
    assert_eq!(issue.done_ratio(), 50);
    assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 3);
}

fn issue_with_description(id: u16, description: &str) -> IssueAggregate {
    let mut issue = sample_issue_aggregate(id, "subject", 1.into(), None, None, None, 0);
    issue.issue.description = description.to_string();
    issue
}

/// Issue 99の詳細取得の完了。直前に`StartFetching`が必要。
fn fetched_issue_99() -> Action {
    Action::IssueFetchSucceeded {
        id: IssueId::new(99),
        issue: sample_issue_aggregate(99, "issue", 1.into(), None, None, None, 0),
        children: vec![],
    }
}
