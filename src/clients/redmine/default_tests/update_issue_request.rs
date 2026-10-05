use super::super::UpdateIssueRequest;
use crate::test_support::local_date;
use crate::vos::{IssueStatusId, UserId};

use crate::clients::redmine::IssueUpdate;

fn request_json(update: &IssueUpdate) -> serde_json::Value {
    serde_json::to_value(UpdateIssueRequest::from(update)).unwrap()
}

#[test]
fn omits_properties_that_are_not_sent() {
    let update = IssueUpdate {
        status_id: Some(IssueStatusId::new(2)),
        ..IssueUpdate::default()
    };

    assert_eq!(
        request_json(&update),
        serde_json::json!({ "issue": { "status_id": 2 } })
    );
}

#[test]
fn sends_set_values_and_clears_values_with_empty_strings() {
    let update = IssueUpdate {
        assigned_to_id: Some(Some(UserId::new(1002))),
        due_date: Some(Some(local_date(2026, 3, 31))),
        category_id: Some(None),
        estimated_hours: Some(None),
        ..IssueUpdate::default()
    };

    assert_eq!(
        request_json(&update),
        serde_json::json!({
            "issue": {
                "assigned_to_id": 1002,
                "due_date": "2026-03-31",
                "category_id": "",
                "estimated_hours": "",
            }
        })
    );
}
