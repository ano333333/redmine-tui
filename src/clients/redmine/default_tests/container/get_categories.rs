use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::EntityIdValue;

#[test]
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
