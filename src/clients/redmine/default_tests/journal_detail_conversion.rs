use super::super::journal_detail_conversion::{RedmineJournalDetail, try_into_domain};
use super::expect_error;
use crate::clients::redmine::RedmineClientError;
use crate::vos::{IssueStatusId, JournalDetailAttr, PriorityId, ProjectId, TrackerId, UserId};

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
