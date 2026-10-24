use super::integration_support::{
    TEST_API_KEY, authenticated_client, expect_not_found, expect_unauthorized, reseed_redmine,
    run_contract, test_error, unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::{
    AttachmentId, EntityIdValue, IssueId, JournalDetail, JournalDetailAttachmentChange, UploadToken,
};

/// seedの添付ファイルはID 1だけのため、次にuploadしたファイルのIDは2になる。
const UPLOADED_ATTACHMENT_ID: u16 = 2;

#[test]
fn attach_upload_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_upload_and_attach_status_codes(&base_url).await?;
        assert_attached_upload_is_fetched_with_journal(&base_url).await?;
        assert_upload_file_401(&base_url).await?;
        assert_attach_upload_404(&base_url).await?;
        Ok(())
    });
}

/// クライアントは成功を2xxで判定するため、Redmineが返す具体的な状態コードをここで確かめる。
async fn assert_upload_and_attach_status_codes(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let http = reqwest::Client::new();
    let upload = http
        .post(format!("{base_url}/uploads.json?filename=status.txt"))
        .header("X-Redmine-API-Key", TEST_API_KEY)
        .header("Content-Type", "application/octet-stream")
        .body("status")
        .send()
        .await?;
    assert_eq!(upload.status().as_u16(), 201);
    let token = upload.json::<serde_json::Value>().await?["upload"]["token"]
        .as_str()
        .ok_or_else(|| test_error("upload response has no token"))?
        .to_string();

    let attach = http
        .put(format!("{base_url}/issues/3.json"))
        .header("X-Redmine-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({
            "issue": { "uploads": [{ "token": token, "filename": "status.txt" }] }
        }))
        .send()
        .await?;
    assert_eq!(attach.status().as_u16(), 204);
    Ok(())
}

async fn assert_attached_upload_is_fetched_with_journal(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 状態コードの確認で添付ファイルIDを1つ使うため、seed直後の状態に戻す。
    reseed_redmine()?;
    let client = authenticated_client(base_url);
    let token = client
        .upload_file("note.txt", b"attached note".to_vec())
        .await
        .map_err(|error| test_error(format!("upload_file returned {error:?}")))?;
    client
        .attach_upload(IssueId::new(1), &token, "renamed.txt")
        .await
        .map_err(|error| test_error(format!("attach_upload returned {error:?}")))?;

    let issue = client
        .get_issue(IssueId::new(1))
        .await
        .map_err(|error| test_error(format!("get_issue(1) returned {error:?}")))?
        .aggregate;
    assert_eq!(
        issue
            .attachments
            .iter()
            .map(|attachment| (
                attachment.id.get(),
                attachment.filename.as_str(),
                attachment.filesize
            ))
            .collect::<Vec<_>>(),
        vec![(UPLOADED_ATTACHMENT_ID, "renamed.txt", 13)]
    );
    let [journal] = issue.journals.as_slice() else {
        return Err(test_error(format!(
            "expected one journal, got {:?}",
            issue.journals
        )));
    };
    assert!(matches!(
        journal.details.as_slice(),
        [JournalDetail::Attachment(detail)]
            if detail.id == AttachmentId::new(UPLOADED_ATTACHMENT_ID)
                && detail.change
                    == JournalDetailAttachmentChange::Added {
                        filename: "renamed.txt".to_string()
                    }
    ));
    Ok(())
}

async fn assert_upload_file_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.upload_file("note.txt", b"note".to_vec()).await).await
}

async fn assert_attach_upload_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(
        client
            .attach_upload(IssueId::new(9999), &UploadToken::new("1.invalid"), "x.txt")
            .await,
    )
    .await
}
