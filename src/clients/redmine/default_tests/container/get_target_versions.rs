use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::EntityIdValue;

#[test]
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
