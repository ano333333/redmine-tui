use reqwest::StatusCode;

use super::super::map_response_status;
use super::{block_on, expect_error, http_error, mount_get};
use crate::clients::redmine::RedmineHttpError;
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};

#[test]
fn get_trackers_maps_invalid_response_json_to_client_error() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(&mock_server, "/trackers.json", 200, r#"{"trackers":["#);
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(block_on(client.get_trackers()), "get_trackers succeeded");

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("error decoding response body"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn redmine_client_error_serializes_for_structured_logs() {
    let error = RedmineClientError::BadRequest {
        context: RedmineHttpError {
            method: "GET".to_string(),
            url: "http://localhost:8080/users.json".to_string(),
            status_code: 400,
            response_body: r#"{"errors":["invalid"]}"#.to_string(),
        },
    };

    let value = serde_json::to_value(error).unwrap();

    assert_eq!(
        value,
        serde_json::json!({
            "type": "BadRequest",
            "context": {
                "method": "GET",
                "url": "http://localhost:8080/users.json",
                "status_code": 400,
                "response_body": "{\"errors\":[\"invalid\"]}",
            }
        })
    );
}

#[test]
#[should_panic(expected = "unexpected Redmine response status: 418")]
fn panics_on_unexpected_response_status() {
    let _ = map_response_status::<()>(
        StatusCode::IM_A_TEAPOT,
        http_error(418, "http://localhost:8080"),
    );
}
