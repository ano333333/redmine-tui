use chrono::{DateTime, Local, NaiveDate, TimeZone};

use super::super::journal_detail_conversion::{RedmineJournalDetail, try_into_domain};
use super::expect_error;
use crate::clients::redmine::RedmineClientError;
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, JournalDetailAttr, PriorityId, ProjectId, TargetVersionId,
    TrackerId, UserId,
};

fn detail(name: &str, old_value: Option<&str>, new_value: Option<&str>) -> RedmineJournalDetail {
    RedmineJournalDetail {
        property: "attr".to_string(),
        name: name.to_string(),
        old_value: old_value.map(str::to_string),
        new_value: new_value.map(str::to_string),
    }
}

#[test]
fn status_id_attr_is_converted() {
    let actual = try_into_domain(detail("status_id", Some("1"), Some("2")))
        .expect("status_id conversion failed");
    let Some(attr) = actual else {
        panic!("status_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::StatusId { old, new } => {
            assert_eq!(old, IssueStatusId::new(1));
            assert_eq!(new, IssueStatusId::new(2));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn tracker_id_attr_is_converted() {
    let actual = try_into_domain(detail("tracker_id", Some("3"), Some("4")))
        .expect("tracker_id conversion failed");
    let Some(attr) = actual else {
        panic!("tracker_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::TrackerId { old, new } => {
            assert_eq!(old, TrackerId::new(3));
            assert_eq!(new, TrackerId::new(4));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn project_id_attr_is_converted() {
    let actual = try_into_domain(detail("project_id", Some("5"), Some("6")))
        .expect("project_id conversion failed");
    let Some(attr) = actual else {
        panic!("project_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::ProjectId { old, new } => {
            assert_eq!(old, ProjectId::new(5));
            assert_eq!(new, ProjectId::new(6));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn subject_attr_is_converted_with_missing_value_as_empty_string() {
    let actual = try_into_domain(detail("subject", Some("Old title"), None))
        .expect("subject conversion failed");
    let Some(attr) = actual else {
        panic!("subject attr was skipped");
    };

    match attr {
        JournalDetailAttr::Subject { old, new } => {
            assert_eq!(old, "Old title");
            assert_eq!(new, "");
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn description_attr_is_converted_with_missing_values_as_empty_strings() {
    let actual =
        try_into_domain(detail("description", None, None)).expect("description conversion failed");
    let Some(attr) = actual else {
        panic!("description attr was skipped");
    };

    match attr {
        JournalDetailAttr::Description { old, new } => {
            assert_eq!(old, "");
            assert_eq!(new, "");
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn priority_id_attr_is_converted() {
    let actual = try_into_domain(detail("priority_id", Some("7"), Some("8")))
        .expect("priority_id conversion failed");
    let Some(attr) = actual else {
        panic!("priority_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::PriorityId { old, new } => {
            assert_eq!(old, PriorityId::new(7));
            assert_eq!(new, PriorityId::new(8));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn author_id_attr_is_converted() {
    let actual = try_into_domain(detail("author_id", Some("9"), Some("10")))
        .expect("author_id conversion failed");
    let Some(attr) = actual else {
        panic!("author_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::AuthorId { old, new } => {
            assert_eq!(old, UserId::new(9));
            assert_eq!(new, UserId::new(10));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn non_attr_property_is_skipped() {
    let mut skip_detail = detail("status_id", Some("1"), Some("2"));
    skip_detail.property = "cf".to_string();

    let actual = try_into_domain(skip_detail).expect("non-attr property conversion failed");
    assert!(actual.is_none());
}

#[test]
fn unsupported_attr_name_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("not_an_attribute", Some("1"), Some("2"))),
        "unsupported attr name was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("not_an_attribute"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn invalid_id_value_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("status_id", Some("not-a-number"), Some("2"))),
        "invalid id value was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("'status_id'"),
                "unexpected reason: {reason}"
            );
            assert!(
                reason.contains("'not-a-number'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

fn local_datetime_at(year: i32, month: u32, day: u32) -> DateTime<Local> {
    let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .unwrap()
}

#[test]
fn category_id_attr_is_converted() {
    let actual = try_into_domain(detail("category_id", Some("1"), Some("2")))
        .expect("category_id conversion failed");
    let Some(attr) = actual else {
        panic!("category_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::CategoryId { old, new } => {
            assert_eq!(old, Some(CategoryId::new(1)));
            assert_eq!(new, Some(CategoryId::new(2)));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn category_id_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual = try_into_domain(detail("category_id", None, Some("")))
        .expect("category_id conversion failed");
    let Some(attr) = actual else {
        panic!("category_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::CategoryId { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn assigned_to_id_attr_is_converted() {
    let actual = try_into_domain(detail("assigned_to_id", Some("3"), Some("4")))
        .expect("assigned_to_id conversion failed");
    let Some(attr) = actual else {
        panic!("assigned_to_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::AssignedToId { old, new } => {
            assert_eq!(old, Some(UserId::new(3)));
            assert_eq!(new, Some(UserId::new(4)));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn assigned_to_id_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual = try_into_domain(detail("assigned_to_id", None, Some("")))
        .expect("assigned_to_id conversion failed");
    let Some(attr) = actual else {
        panic!("assigned_to_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::AssignedToId { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn fixed_version_id_attr_is_converted() {
    let actual = try_into_domain(detail("fixed_version_id", Some("5"), Some("6")))
        .expect("fixed_version_id conversion failed");
    let Some(attr) = actual else {
        panic!("fixed_version_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::FixedVersionId { old, new } => {
            assert_eq!(old, Some(TargetVersionId::new(5)));
            assert_eq!(new, Some(TargetVersionId::new(6)));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn fixed_version_id_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual = try_into_domain(detail("fixed_version_id", None, Some("")))
        .expect("fixed_version_id conversion failed");
    let Some(attr) = actual else {
        panic!("fixed_version_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::FixedVersionId { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn start_date_attr_is_converted() {
    let actual = try_into_domain(detail("start_date", Some("2026-09-01"), Some("2026-09-10")))
        .expect("start_date conversion failed");
    let Some(attr) = actual else {
        panic!("start_date attr was skipped");
    };

    match attr {
        JournalDetailAttr::StartDate { old, new } => {
            assert_eq!(old, Some(local_datetime_at(2026, 9, 1)));
            assert_eq!(new, Some(local_datetime_at(2026, 9, 10)));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn start_date_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual = try_into_domain(detail("start_date", None, Some("")))
        .expect("start_date conversion failed");
    let Some(attr) = actual else {
        panic!("start_date attr was skipped");
    };

    match attr {
        JournalDetailAttr::StartDate { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn due_date_attr_is_converted() {
    let actual = try_into_domain(detail("due_date", Some("2026-09-02"), Some("2026-09-11")))
        .expect("due_date conversion failed");
    let Some(attr) = actual else {
        panic!("due_date attr was skipped");
    };

    match attr {
        JournalDetailAttr::DueDate { old, new } => {
            assert_eq!(old, Some(local_datetime_at(2026, 9, 2)));
            assert_eq!(new, Some(local_datetime_at(2026, 9, 11)));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn due_date_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual =
        try_into_domain(detail("due_date", None, Some(""))).expect("due_date conversion failed");
    let Some(attr) = actual else {
        panic!("due_date attr was skipped");
    };

    match attr {
        JournalDetailAttr::DueDate { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn done_ratio_attr_is_converted() {
    let actual = try_into_domain(detail("done_ratio", Some("30"), Some("50")))
        .expect("done_ratio conversion failed");
    let Some(attr) = actual else {
        panic!("done_ratio attr was skipped");
    };

    match attr {
        JournalDetailAttr::DoneRatio { old, new } => {
            assert_eq!(old, 30);
            assert_eq!(new, 50);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn estimated_hours_attr_is_converted_from_whole_float_values() {
    let actual = try_into_domain(detail("estimated_hours", Some("1.0"), Some("2")))
        .expect("estimated_hours conversion failed");
    let Some(attr) = actual else {
        panic!("estimated_hours attr was skipped");
    };

    match attr {
        JournalDetailAttr::EstimatedHours { old, new } => {
            assert_eq!(old, Some(1));
            assert_eq!(new, Some(2));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn estimated_hours_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual = try_into_domain(detail("estimated_hours", None, Some("")))
        .expect("estimated_hours conversion failed");
    let Some(attr) = actual else {
        panic!("estimated_hours attr was skipped");
    };

    match attr {
        JournalDetailAttr::EstimatedHours { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn parent_id_attr_is_converted() {
    let actual = try_into_domain(detail("parent_id", Some("7"), Some("8")))
        .expect("parent_id conversion failed");
    let Some(attr) = actual else {
        panic!("parent_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::ParentId { old, new } => {
            assert_eq!(old, Some(IssueId::new(7)));
            assert_eq!(new, Some(IssueId::new(8)));
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn parent_id_attr_with_missing_or_empty_values_is_converted_to_none() {
    let actual =
        try_into_domain(detail("parent_id", None, Some(""))).expect("parent_id conversion failed");
    let Some(attr) = actual else {
        panic!("parent_id attr was skipped");
    };

    match attr {
        JournalDetailAttr::ParentId { old, new } => {
            assert_eq!(old, None);
            assert_eq!(new, None);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn is_private_attr_is_converted() {
    let actual = try_into_domain(detail("is_private", Some("0"), Some("1")))
        .expect("is_private conversion failed");
    let Some(attr) = actual else {
        panic!("is_private attr was skipped");
    };

    match attr {
        JournalDetailAttr::IsPrivate { old, new } => {
            assert_eq!(old, false);
            assert_eq!(new, true);
        }
        _ => panic!("unexpected attr variant"),
    }
}

#[test]
fn invalid_category_id_value_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("category_id", Some("not-a-number"), Some("2"))),
        "invalid category_id value was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("'category_id'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn invalid_start_date_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("start_date", Some("2026-09-01"), Some("not-a-date"))),
        "invalid start_date was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(reason.contains("not-a-date"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn invalid_due_date_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("due_date", Some("not-a-date"), Some("2026-09-11"))),
        "invalid due_date was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(reason.contains("not-a-date"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn missing_done_ratio_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("done_ratio", None, Some("50"))),
        "missing done_ratio was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("'done_ratio'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn invalid_is_private_value_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("is_private", Some("maybe"), Some("1"))),
        "invalid is_private value was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("'is_private'"),
                "unexpected reason: {reason}"
            );
            assert!(reason.contains("'maybe'"), "unexpected reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn fractional_estimated_hours_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("estimated_hours", Some("1.5"), Some("2"))),
        "fractional estimated_hours was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("'estimated_hours'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn out_of_range_estimated_hours_maps_to_client_error() {
    let actual_error = expect_error(
        try_into_domain(detail("estimated_hours", Some("70000"), Some("2"))),
        "out-of-range estimated_hours was converted",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("'estimated_hours'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
