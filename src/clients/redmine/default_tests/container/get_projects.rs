use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::EntityIdValue;

#[test]
fn get_projects_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_projects_200(&base_url).await?;
        assert_get_projects_401(&base_url).await?;
        assert_get_projects_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_projects_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let projects = authenticated_client(base_url)
        .get_projects()
        .await
        .map_err(|error| test_error(format!("get_projects returned {error:?}")))?;

    assert!(
        projects
            .iter()
            .any(|project| project.id.get() == 1 && project.name == "Sample Project"),
        "expected Sample Project id 1, got {:?}",
        projects
            .iter()
            .map(|project| (project.id.get(), project.name.as_str()))
            .collect::<Vec<_>>()
    );

    Ok(())
}

async fn assert_get_projects_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_projects().await).await
}

async fn assert_get_projects_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_projects().await).await
}
