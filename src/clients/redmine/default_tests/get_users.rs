use reqwest::StatusCode;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error, http_error};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};

#[test]
fn get_users_maps_known_redmine_error_statuses_with_response_context() {
    let cases = [
        (StatusCode::BAD_REQUEST, "BadRequest"),
        (StatusCode::UNAUTHORIZED, "Unauthorized"),
        (StatusCode::FORBIDDEN, "NotFound"),
        (StatusCode::NOT_FOUND, "NotFound"),
        (StatusCode::UNPROCESSABLE_ENTITY, "UnprocessableEntity"),
        (StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError"),
        (StatusCode::SERVICE_UNAVAILABLE, "InternalServerError"),
    ];

    for (status, expected_error_type) in cases {
        let mock_server = block_on(MockServer::start());
        block_on(
            Mock::given(method("GET"))
                .and(path("/users.json"))
                .respond_with(
                    ResponseTemplate::new(status.as_u16()).set_body_string(r#"{"error":"failed"}"#),
                )
                .expect(1)
                .mount(&mock_server),
        );
        let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

        let actual_error = expect_error(block_on(client.get_users()), "get_users succeeded");
        let expected_context = http_error(status.as_u16(), &mock_server.uri());

        match actual_error {
            RedmineClientError::BadRequest { context } => {
                assert_eq!(expected_error_type, "BadRequest");
                assert_eq!(context, expected_context);
            }
            RedmineClientError::Unauthorized { context } => {
                assert_eq!(expected_error_type, "Unauthorized");
                assert_eq!(context, expected_context);
            }
            RedmineClientError::NotFound { context } => {
                assert_eq!(expected_error_type, "NotFound");
                assert_eq!(context, expected_context);
            }
            RedmineClientError::UnprocessableEntity { context } => {
                assert_eq!(expected_error_type, "UnprocessableEntity");
                assert_eq!(context, expected_context);
            }
            RedmineClientError::InternalServerError { context } => {
                assert_eq!(expected_error_type, "InternalServerError");
                assert_eq!(context, expected_context);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

#[test]
fn get_users_maps_invalid_response_json_to_client_error() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/users.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"users":["#))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(block_on(client.get_users()), "get_users succeeded");

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
