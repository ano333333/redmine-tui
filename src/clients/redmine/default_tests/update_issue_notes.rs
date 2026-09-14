use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{
    DefaultRedmineClient, RedmineClient, RedmineClientError, RedmineHttpError,
};
use crate::vos::IssueId;

#[test]
fn update_issue_notes_sends_redmine_put_request() {
    let mock_server = block_on(MockServer::start());
    let expected_body = json!({
        "issue": {
            "notes": "new issue notes"
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

    block_on(client.update_issue_notes(IssueId::new(42), "new issue notes")).unwrap();
}

#[test]
fn update_issue_notes_maps_unauthorized_with_response_context() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("PUT"))
            .and(path("/issues/42.json"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"failed"}"#))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(
        block_on(client.update_issue_notes(IssueId::new(42), "new issue notes")),
        "update_issue_notes succeeded",
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
