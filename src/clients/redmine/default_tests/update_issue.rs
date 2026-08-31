use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{
    DefaultRedmineClient, RedmineClient, RedmineClientError, RedmineHttpError,
};
use crate::test_support::sample_issue_aggregate;
use crate::vos::IssueStatusId;

#[test]
fn update_issue_sends_redmine_put_request() {
    let mock_server = block_on(MockServer::start());
    let issue = sample_issue_aggregate(
        42,
        "Fix login",
        IssueStatusId::new(3),
        Some(1001),
        Some("2026-08-19T00:00:00+09:00"),
        Some("2026-08-31T00:00:00+09:00"),
        30,
    );
    let expected_body = json!({
        "issue": {
            "subject": "Fix login",
            "description": "body",
            "status_id": 3,
            "priority_id": 1,
            "assigned_to_id": 1001,
            "fixed_version_id": 1,
            "start_date": "2026-08-19",
            "due_date": "2026-08-31",
            "done_ratio": 30,
            "estimated_hours": 8,
            "category_id": 1
        }
    });
    block_on(
        Mock::given(method("PUT"))
            .and(path("/issues/42.json"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .and(header("Content-Type", "application/json"))
            .and(body_json(expected_body))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    block_on(client.update_issue(&issue)).unwrap();
}

#[test]
fn update_issue_maps_unauthorized_with_response_context() {
    let mock_server = block_on(MockServer::start());
    let issue = sample_issue_aggregate(
        42,
        "Fix login",
        IssueStatusId::new(3),
        Some(1001),
        Some("2026-08-19T00:00:00+09:00"),
        Some("2026-08-31T00:00:00+09:00"),
        30,
    );
    block_on(
        Mock::given(method("PUT"))
            .and(path("/issues/42.json"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"failed"}"#))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(
        block_on(client.update_issue(&issue)),
        "update_issue succeeded",
    );

    match actual_error {
        RedmineClientError::Unauthorized { context } => assert_eq!(
            context,
            RedmineHttpError {
                method: "PUT".to_string(),
                url: format!("{}/issues/42.json", mock_server.uri()),
                status_code: 401,
                response_body: r#"{"error":"failed"}"#.to_string(),
            }
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}
