use super::integration_support::{TEST_API_KEY, reseed_redmine, run_contract, test_error};

/// seedのIssueはID 1〜3のため、AUTO_INCREMENTがリセットされていれば次に作成されるIDは4になる。
const FIRST_CREATED_ISSUE_ID: u64 = 4;

#[test]
fn reseed_resets_created_issue_id_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_eq!(create_issue(&base_url).await?, FIRST_CREATED_ISSUE_ID);

        reseed_redmine()?;

        assert_eq!(create_issue(&base_url).await?, FIRST_CREATED_ISSUE_ID);
        Ok(())
    });
}

async fn create_issue(base_url: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/issues.json"))
        .header("X-Redmine-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({
            "issue": {
                "project_id": 1,
                "tracker_id": 1,
                "status_id": 1,
                "priority_id": 1,
                "subject": "created after reseed",
            }
        }))
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(test_error(format!(
            "create issue returned {status}: {body}"
        )));
    }
    body["issue"]["id"]
        .as_u64()
        .ok_or_else(|| test_error(format!("create issue response has no id: {body}")))
}
