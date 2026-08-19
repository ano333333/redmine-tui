use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use super::{block_on, mount_get_paginated};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient};
use crate::vos::EntityIdValue;

#[test]
fn get_target_versions_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get_paginated(
        &mock_server,
        "/projects.json",
        200,
        r#"{"projects":[{"id":10,"name":"Redmine TUI"}]}"#,
    );
    mount_get_paginated(
        &mock_server,
        "/projects/10/versions.json",
        200,
        r#"{"versions":[{"id":7,"name":"v1.0"}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let versions = block_on(client.get_target_versions()).unwrap();

    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].id.get(), 7);
    assert_eq!(versions[0].name, "v1.0");
}

#[test]
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_target_versions_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_target_versions_200(&base_url).await?;
        assert_get_target_versions_401(&base_url).await?;
        assert_get_target_versions_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_target_versions_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let versions = authenticated_client(base_url)
        .get_target_versions()
        .await
        .map_err(|error| test_error(format!("get_target_versions returned {error:?}")))?;

    if !versions
        .iter()
        .any(|version| version.id.get() == 1 && version.name == "v1.2.3")
    {
        return Err(test_error(format!(
            "expected seeded v1.2.3 among {} versions",
            versions.len()
        )));
    }

    Ok(())
}

async fn assert_get_target_versions_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_target_versions().await).await
}

async fn assert_get_target_versions_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_target_versions().await).await
}
