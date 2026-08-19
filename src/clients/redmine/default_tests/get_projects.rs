use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use super::{block_on, mount_get_paginated};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient};
use crate::vos::EntityIdValue;

#[test]
fn get_projects_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get_paginated(
        &mock_server,
        "/projects.json",
        200,
        r#"{"projects":[{"id":10,"name":"Redmine TUI"}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let projects = block_on(client.get_projects()).unwrap();

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id.get(), 10);
    assert_eq!(projects[0].name, "Redmine TUI");
}

#[test]
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
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
