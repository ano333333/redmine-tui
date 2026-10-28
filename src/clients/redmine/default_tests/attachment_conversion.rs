use super::super::attachment_conversion::{
    RedmineAttachment, RedmineAttachmentAuthor, convert_attachment,
};
use super::expect_error;
use crate::clients::redmine::RedmineClientError;
use crate::vos::AttachmentId;

fn sample_attachment() -> RedmineAttachment {
    RedmineAttachment {
        id: 12,
        filename: "sample.png".to_string(),
        filesize: 1024,
        content_type: Some("image/png".to_string()),
        description: Some("screenshot".to_string()),
        content_url: "http://localhost:3000/attachments/download/12/sample.png".to_string(),
        thumbnail_url: Some("http://localhost:3000/attachments/thumbnail/12".to_string()),
        author: RedmineAttachmentAuthor {
            name: "Taro Yamada".to_string(),
        },
        created_on: "2026-01-02T03:04:05+00:00".to_string(),
    }
}

#[test]
fn valid_redmine_attachment_is_converted() {
    let expected_created_on = chrono::DateTime::parse_from_rfc3339("2026-01-02T03:04:05+00:00")
        .unwrap()
        .with_timezone(&chrono::Local);

    let attachment = convert_attachment(sample_attachment()).expect("conversion failed");

    assert_eq!(attachment.id, AttachmentId::new(12));
    assert_eq!(attachment.filename, "sample.png");
    assert_eq!(attachment.filesize, 1024);
    assert_eq!(attachment.content_type.as_deref(), Some("image/png"));
    assert_eq!(attachment.description, "screenshot");
    assert_eq!(
        attachment.content_url,
        "http://localhost:3000/attachments/download/12/sample.png"
    );
    assert!(attachment.thumbnailable);
    assert_eq!(attachment.author, "Taro Yamada");
    assert_eq!(attachment.created_on, expected_created_on);
}

#[test]
fn empty_content_type_is_converted_to_none() {
    let mut attachment = sample_attachment();
    attachment.content_type = Some(String::new());

    let attachment = convert_attachment(attachment).expect("conversion failed");

    assert_eq!(attachment.content_type, None);
}

#[test]
fn missing_thumbnail_url_is_converted_to_not_thumbnailable() {
    let mut attachment = sample_attachment();
    attachment.thumbnail_url = None;

    let attachment = convert_attachment(attachment).expect("conversion failed");

    assert!(!attachment.thumbnailable);
}

#[test]
fn null_description_is_converted_to_empty_string() {
    let mut attachment = sample_attachment();
    attachment.description = None;

    let attachment = convert_attachment(attachment).expect("conversion failed");

    assert_eq!(attachment.description, "");
}

#[test]
fn invalid_created_on_returns_client_error() {
    let mut attachment = sample_attachment();
    attachment.created_on = "2026/01/02".to_string();

    let error = expect_error(
        convert_attachment(attachment),
        "invalid created_on was accepted",
    );

    assert!(matches!(error, RedmineClientError::Client { .. }));
}
