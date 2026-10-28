use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::AttachmentId;

const PNG_HEADER: [u8; 4] = [0x89, 0x50, 0x4e, 0x47];

#[test]
fn get_attachment_thumbnail_gets_bytes_by_id_and_size() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/attachments/thumbnail/12/800"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_HEADER.to_vec()))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let content =
        block_on(client.get_attachment_thumbnail(AttachmentId::new(12), 800, 1024)).unwrap();

    assert_eq!(content, PNG_HEADER.to_vec());
}

#[test]
fn get_attachment_content_gets_bytes_by_id() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/attachments/download/12"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_HEADER.to_vec()))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let content = block_on(client.get_attachment_content(AttachmentId::new(12), 1024)).unwrap();

    assert_eq!(content, PNG_HEADER.to_vec());
}

#[test]
fn get_attachment_content_accepts_exactly_max_len_bytes() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/attachments/download/12"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_HEADER.to_vec()))
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let content = block_on(client.get_attachment_content(AttachmentId::new(12), 4)).unwrap();

    assert_eq!(content.len(), 4);
}

#[test]
fn get_attachment_content_rejects_responses_over_max_len() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/attachments/download/12"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_HEADER.to_vec()))
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let error = expect_error(
        block_on(client.get_attachment_content(AttachmentId::new(12), 3)),
        "oversized attachment was accepted",
    );

    assert!(matches!(error, RedmineClientError::Client { .. }));
}

#[test]
fn get_attachment_thumbnail_maps_not_found() {
    let mock_server = block_on(MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/attachments/thumbnail/12/800"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let error = expect_error(
        block_on(client.get_attachment_thumbnail(AttachmentId::new(12), 800, 1024)),
        "missing thumbnail was returned",
    );

    assert!(matches!(error, RedmineClientError::NotFound { .. }));
}
