use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::{EntityIdValue, IssueId};

#[test]
fn get_issue_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_issue_200(&base_url).await?;
        assert_get_issue_401(&base_url).await?;
        assert_get_issue_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_issue_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fetched = authenticated_client(base_url)
        .get_issue(IssueId::new(1))
        .await
        .map_err(|error| test_error(format!("get_issue(1) returned {error:?}")))?;

    assert_eq!(fetched.aggregate.issue.id.get(), 1);
    assert_eq!(fetched.aggregate.issue.subject, "issue1");
    assert_eq!(fetched.aggregate.author_id.get(), 1001);
    assert_eq!(fetched.aggregate.issue.project_id.get(), 1);

    Ok(())
}

async fn assert_get_issue_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_issue(IssueId::new(1)).await).await
}

async fn assert_get_issue_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(client.get_issue(IssueId::new(9999)).await).await
}
