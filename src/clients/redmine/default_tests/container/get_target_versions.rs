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
    let values = authenticated_client(base_url)
        .get_target_versions()
        .await
        .map_err(|error| test_error(format!("get_target_versions returned {error:?}")))?;

    assert_eq!(
        values
            .iter()
            .map(|value| (value.id.get(), value.name.as_str(), value.project_id.get()))
            .collect::<Vec<_>>(),
        vec![(1, "v1.2.3", 1), (2, "v2.0.0", 1), (3, "p2-v1.0.0", 2)]
    );

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
