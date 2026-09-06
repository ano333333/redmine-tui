use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use super::{block_on, mount_get_paginated};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient};
use crate::vos::EntityIdValue;

#[test]
fn get_categories_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get_paginated(
        &mock_server,
        "/projects.json",
        200,
        r#"{"projects":[{"id":10,"name":"Redmine TUI"}]}"#,
    );
    mount_get_paginated(
        &mock_server,
        "/projects/10/issue_categories.json",
        200,
        r#"{"issue_categories":[{"id":4,"name":"Backend","project":{"id":10,"name":"Redmine TUI"}}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let categories = block_on(client.get_categories()).unwrap();

    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].id.get(), 4);
    assert_eq!(categories[0].name, "Backend");
    assert_eq!(categories[0].project_id.get(), 10);
}

#[test]
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_categories_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_categories_200(&base_url).await?;
        assert_get_categories_401(&base_url).await?;
        assert_get_categories_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_categories_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let categories = authenticated_client(base_url)
        .get_categories()
        .await
        .map_err(|error| test_error(format!("get_categories returned {error:?}")))?;

    if !categories
        .iter()
        .any(|category| category.id.get() == 1 && category.name == "category1")
    {
        return Err(test_error(format!(
            "expected seeded category1 among {} categories",
            categories.len()
        )));
    }

    Ok(())
}

async fn assert_get_categories_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_categories().await).await
}

async fn assert_get_categories_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_categories().await).await
}
