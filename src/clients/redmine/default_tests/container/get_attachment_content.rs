use super::integration_support::{
    authenticated_client, expect_not_found, run_contract, test_error,
};
use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::vos::AttachmentId;

/// seedの添付ファイル1は、Issue 3に添付した`datas/attachment_files/sample.png`である。
const SAMPLE_PNG: &[u8] = include_bytes!("../../../../../datas/attachment_files/sample.png");
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const MAX_LEN: u64 = 16 * 1024 * 1024;

#[test]
fn get_attachment_content_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_attachment_thumbnail_200(&base_url).await?;
        assert_get_attachment_content_200(&base_url).await?;
        assert_get_attachment_content_over_max_len(&base_url).await?;
        assert_get_attachment_thumbnail_404(&base_url).await?;
        assert_get_attachment_content_404(&base_url).await?;
        Ok(())
    });
}

/// テスト用のRedmineはImageMagickを持つため、PNGのサムネイルを返す。
async fn assert_get_attachment_thumbnail_200(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let thumbnail = authenticated_client(base_url)
        .get_attachment_thumbnail(AttachmentId::new(1), 800, MAX_LEN)
        .await
        .map_err(|error| test_error(format!("get_attachment_thumbnail(1) returned {error:?}")))?;

    assert!(thumbnail.starts_with(PNG_SIGNATURE));
    assert!(thumbnail.len() < SAMPLE_PNG.len());
    Ok(())
}

async fn assert_get_attachment_content_200(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = authenticated_client(base_url)
        .get_attachment_content(AttachmentId::new(1), MAX_LEN)
        .await
        .map_err(|error| test_error(format!("get_attachment_content(1) returned {error:?}")))?;

    assert!(content == SAMPLE_PNG);
    Ok(())
}

async fn assert_get_attachment_content_over_max_len(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = authenticated_client(base_url)
        .get_attachment_content(AttachmentId::new(1), SAMPLE_PNG.len() as u64 - 1)
        .await;

    match result {
        Err(RedmineClientError::Client { .. }) => Ok(()),
        other => Err(test_error(format!(
            "expected Client error, got {:?}",
            other.map(|content| content.len())
        ))),
    }
}

async fn assert_get_attachment_thumbnail_404(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(
        client
            .get_attachment_thumbnail(AttachmentId::new(9999), 800, MAX_LEN)
            .await,
    )
    .await
}

async fn assert_get_attachment_content_404(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(
        client
            .get_attachment_content(AttachmentId::new(9999), MAX_LEN)
            .await,
    )
    .await
}
