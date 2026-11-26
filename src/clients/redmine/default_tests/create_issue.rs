use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::base::NewIssueUpload;
use crate::clients::redmine::{DefaultRedmineClient, NewIssue, RedmineClient, RedmineClientError};
use crate::test_support::local_date;
use crate::vos::{IssueId, PriorityId, ProjectId, TrackerId, UploadToken, UserId};

fn new_issue() -> NewIssue {
    NewIssue {
        project_id: ProjectId::new(1),
        subject: "新しいIssue".to_string(),
        parent_issue_id: None,
        description: None,
        tracker_id: None,
        status_id: None,
        priority_id: None,
        assigned_to_id: None,
        target_version_id: None,
        start_date: None,
        due_date: None,
        done_ratio: None,
        estimated_hours: None,
        category_id: None,
        uploads: Vec::new(),
    }
}

fn mount_create(mock_server: &MockServer, body: serde_json::Value) {
    block_on(
        Mock::given(method("POST"))
            .and(path("/issues.json"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .and(body_json(body))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_string(r#"{"issue": {"id": 99, "subject": "新しいIssue"}}"#),
            )
            .expect(1)
            .mount(mock_server),
    );
}

#[test]
fn create_issue_omits_unset_properties_and_returns_the_created_id() {
    let mock_server = block_on(MockServer::start());
    mount_create(
        &mock_server,
        serde_json::json!({ "issue": { "project_id": 1, "subject": "新しいIssue" } }),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let id = block_on(client.create_issue(&new_issue())).unwrap();

    assert_eq!(id, IssueId::new(99));
}

#[test]
fn create_issue_sends_set_properties_parent_and_uploads() {
    let mock_server = block_on(MockServer::start());
    mount_create(
        &mock_server,
        serde_json::json!({
            "issue": {
                "project_id": 1,
                "subject": "新しいIssue",
                "parent_issue_id": 42,
                "description": "本文",
                "tracker_id": 2,
                "priority_id": 3,
                "assigned_to_id": 1001,
                "start_date": "2026-11-21",
                "done_ratio": 30,
                "estimated_hours": 1.5,
                "uploads": [{ "token": "7.abcdef", "filename": "screen.png" }]
            }
        }),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");
    let issue = NewIssue {
        parent_issue_id: Some(IssueId::new(42)),
        description: Some("本文".to_string()),
        tracker_id: Some(TrackerId::new(2)),
        priority_id: Some(PriorityId::new(3)),
        assigned_to_id: Some(UserId::new(1001)),
        start_date: Some(local_date(2026, 11, 21)),
        done_ratio: Some(30),
        estimated_hours: Some(1.5),
        uploads: vec![NewIssueUpload {
            token: UploadToken::new("7.abcdef"),
            filename: "screen.png".to_string(),
        }],
        ..new_issue()
    };

    block_on(client.create_issue(&issue)).unwrap();
}

#[test]
fn create_issue_maps_unprocessable_entity() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("POST"))
            .and(path("/issues.json"))
            .respond_with(
                ResponseTemplate::new(422)
                    .set_body_string(r#"{"errors": ["Subject cannot be blank"]}"#),
            )
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let error = expect_error(
        block_on(client.create_issue(&new_issue())),
        "invalid issue was created",
    );

    assert!(matches!(
        error,
        RedmineClientError::UnprocessableEntity { .. }
    ));
}
