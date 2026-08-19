use reqwest::StatusCode;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use super::{block_on, expect_error, http_error};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::EntityIdValue;

#[test]
fn get_users_sends_api_token_and_maps_success_response() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/users.json"))
            .and(wiremock::matchers::header(
                "X-Redmine-API-Key",
                "secret-token",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"users":[{"id":1000,"firstname":"Alice","lastname":"Sato"}]}"#,
            ))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let users = block_on(client.get_users()).unwrap();

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].id.get(), 1000);
    assert_eq!(users[0].name, "Alice Sato");
}

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

#[test]
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_users_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_users_200(&base_url).await?;
        assert_get_users_401(&base_url).await?;
        assert_get_users_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_users_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let users = authenticated_client(base_url)
        .get_users()
        .await
        .map_err(|error| test_error(format!("get_users returned {error:?}")))?;

    let expected_names = ["user1 Fixture", "user2 Fixture"];
    assert!(
        users
            .iter()
            .any(|user| expected_names.contains(&user.name.as_str())),
        "expected one of {expected_names:?}, got {:?}",
        users
            .iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>()
    );

    Ok(())
}

async fn assert_get_users_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_users().await).await
}

async fn assert_get_users_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_users().await).await
}
