use super::integration_support::{
    TEST_API_KEY, authenticated_client, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::base::NewIssueUpload;
use crate::clients::redmine::{NewIssue, RedmineClient, RedmineClientError};
use crate::test_support::local_date;
use crate::vos::{EntityIdValue, IssueId, PriorityId, ProjectId, TrackerId, UserId};

#[test]
fn create_issue_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_create_issue_status_code(&base_url).await?;
        assert_created_child_is_fetched_with_properties_and_uploads(&base_url).await?;
        assert_create_issue_422(&base_url).await?;
        assert_create_issue_401(&base_url).await?;
        Ok(())
    });
}

fn new_issue(subject: &str) -> NewIssue {
    NewIssue {
        project_id: ProjectId::new(1),
        subject: subject.to_string(),
        parent_issue_id: None,
        description: None,
        tracker_id: None,
        status_id: None,
        priority_id: None,
        assigned_to_id: None,
        target_version_id: None,
        start_date: None,
        due_date: None,
        done_ratio: None,
        estimated_hours: None,
        category_id: None,
        uploads: Vec::new(),
    }
}

/// クライアントは成功を2xxで判定するため、Redmineが返す具体的な状態コードをここで確かめる。
async fn assert_create_issue_status_code(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/issues.json"))
        .header("X-Redmine-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "issue": { "project_id": 1, "subject": "status" } }))
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 201);
    Ok(())
}

async fn assert_created_child_is_fetched_with_properties_and_uploads(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    let token = client
        .upload_file("note.txt", b"attached note".to_vec())
        .await
        .map_err(|error| test_error(format!("upload_file returned {error:?}")))?;
    let id = client
        .create_issue(&NewIssue {
            parent_issue_id: Some(IssueId::new(4)),
            description: Some("本文".to_string()),
            tracker_id: Some(TrackerId::new(2)),
            priority_id: Some(PriorityId::new(2)),
            assigned_to_id: Some(UserId::new(1001)),
            start_date: Some(local_date(2026, 11, 21)),
            estimated_hours: Some(1.5),
            uploads: vec![NewIssueUpload {
                token,
                filename: "renamed.txt".to_string(),
            }],
            ..new_issue("created child")
        })
        .await
        .map_err(|error| test_error(format!("create_issue returned {error:?}")))?;

    let created = client
        .get_issue(id)
        .await
        .map_err(|error| test_error(format!("get_issue({id}) returned {error:?}")))?
        .aggregate;
    assert_eq!(created.issue.subject, "created child");
    assert_eq!(created.issue.description, "本文");
    assert_eq!(created.parent_id, Some(IssueId::new(4)));
    assert_eq!(created.tracker_id.get(), 2);
    assert_eq!(created.priority_id.get(), 2);
    assert_eq!(created.assigned_to_id, Some(UserId::new(1001)));
    assert_eq!(created.start_date, Some(local_date(2026, 11, 21)));
    assert_eq!(created.estimated_hours, Some(1.5));
    assert_eq!(
        created
            .attachments
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<Vec<_>>(),
        vec!["renamed.txt"]
    );

    let parent = client
        .get_issue(IssueId::new(4))
        .await
        .map_err(|error| test_error(format!("get_issue(4) returned {error:?}")))?;
    assert!(parent.children.iter().any(|child| child.id == id));
    Ok(())
}

async fn assert_create_issue_422(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    match authenticated_client(base_url)
        .create_issue(&new_issue(""))
        .await
    {
        Err(RedmineClientError::UnprocessableEntity { context }) => {
            assert_eq!(context.status_code, 422);
            Ok(())
        }
        other => Err(test_error(format!(
            "expected UnprocessableEntity, got {other:?}"
        ))),
    }
}

async fn assert_create_issue_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.create_issue(&new_issue("unauthorized")).await).await
}
