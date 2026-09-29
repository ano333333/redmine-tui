use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use crate::clients::redmine::RedmineClient;

#[test]
fn get_users_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_users_200(&base_url).await?;
        assert_get_users_401(&base_url).await?;
        assert_get_users_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_users_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let users = authenticated_client(base_url)
        .get_users()
        .await
        .map_err(|error| test_error(format!("get_users returned {error:?}")))?;

    let expected_names = ["user1 Fixture", "user2 Fixture"];
    assert!(
        users
            .iter()
            .any(|user| expected_names.contains(&user.name.as_str())),
        "expected one of {expected_names:?}, got {:?}",
        users
            .iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>()
    );

    Ok(())
}

async fn assert_get_users_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_users().await).await
}

async fn assert_get_users_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_users().await).await
}
