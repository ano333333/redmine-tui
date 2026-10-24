use wiremock::matchers::{body_bytes, body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::{IssueId, UploadToken};

#[test]
fn upload_file_posts_raw_bytes_and_returns_token() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("POST"))
            .and(path("/uploads.json"))
            .and(query_param("filename", "スクリーン ショット.png"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .and(header("Content-Type", "application/octet-stream"))
            .and(body_bytes(vec![0x89, 0x50, 0x4e, 0x47]))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_string(r#"{"upload": {"id": 7, "token": "7.abcdef"}}"#),
            )
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let token =
        block_on(client.upload_file("スクリーン ショット.png", vec![0x89, 0x50, 0x4e, 0x47]))
            .unwrap();

    assert_eq!(token, UploadToken::new("7.abcdef"));
}

#[test]
fn upload_file_maps_unprocessable_entity() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("POST"))
            .and(path("/uploads.json"))
            .respond_with(ResponseTemplate::new(422).set_body_string(
                r#"{"errors": ["This file cannot be uploaded because it exceeds the maximum allowed file size"]}"#,
            ))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let error = expect_error(
        block_on(client.upload_file("large.bin", vec![0; 4])),
        "oversized upload was accepted",
    );

    assert!(matches!(
        error,
        RedmineClientError::UnprocessableEntity { .. }
    ));
}

#[test]
fn attach_upload_sends_only_uploads() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("PUT"))
            .and(path("/issues/42.json"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .and(body_json(serde_json::json!({
                "issue": {
                    "uploads": [{ "token": "7.abcdef", "filename": "screen.png" }]
                }
            })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    block_on(client.attach_upload(
        IssueId::new(42),
        &UploadToken::new("7.abcdef"),
        "screen.png",
    ))
    .unwrap();
}
