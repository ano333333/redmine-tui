use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{
    DefaultRedmineClient, RedmineClient, RedmineClientError, RedmineHttpError,
};
use crate::vos::JournalId;

#[test]
fn update_journal_notes_sends_redmine_put_request() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("PUT"))
            .and(path("/journals/42.json"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .and(header("Content-Type", "application/json"))
            .and(body_json(json!({"journal": {"notes": "updated notes"}})))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    block_on(client.update_journal_notes(JournalId::new(42), "updated notes")).unwrap();
}

#[test]
fn update_journal_notes_maps_unauthorized_with_response_context() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("PUT"))
            .and(path("/journals/42.json"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"failed"}"#))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(
        block_on(client.update_journal_notes(JournalId::new(42), "updated notes")),
        "update_journal_notes succeeded",
    );

    match actual_error {
        RedmineClientError::Unauthorized { context } => assert_eq!(
            context,
            RedmineHttpError {
                method: "PUT".to_string(),
                url: format!("{}/journals/42.json", mock_server.uri()),
                status_code: 401,
                response_body: r#"{"error":"failed"}"#.to_string(),
            }
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn update_journal_notes_maps_connection_failure_to_network_error() {
    let client = DefaultRedmineClient::new("http://127.0.0.1:0", "secret-token");

    let actual_error = expect_error(
        block_on(client.update_journal_notes(JournalId::new(42), "updated notes")),
        "update_journal_notes succeeded",
    );

    match actual_error {
        RedmineClientError::Network { reason } => assert!(!reason.is_empty()),
        other => panic!("unexpected error: {other:?}"),
    }
}
