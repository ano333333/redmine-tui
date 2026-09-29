use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::EntityIdValue;

#[test]
fn get_issue_statuses_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_issue_statuses_200(&base_url).await?;
        assert_get_issue_statuses_401(&base_url).await?;
        assert_get_issue_statuses_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_issue_statuses_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let values = authenticated_client(base_url)
        .get_issue_statuses()
        .await
        .map_err(|error| test_error(format!("get_issue_statuses returned {error:?}")))?;

    assert_eq!(
        values
            .iter()
            .map(|value| (value.id.get(), value.name.as_str(), value.is_closed))
            .collect::<Vec<_>>(),
        vec![
            (1, "新規(new)", false),
            (2, "割り当て(assigned)", false),
            (3, "進行中(accepted)", false),
            (4, "レビュー(review)", false),
            (5, "完了(closed)", true),
            (6, "改修確認待ち", false),
        ]
    );

    Ok(())
}

async fn assert_get_issue_statuses_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_issue_statuses().await).await
}

async fn assert_get_issue_statuses_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_issue_statuses().await).await
}

#[test]
fn get_priorities_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_priorities_200(&base_url).await?;
        assert_get_priorities_401(&base_url).await?;
        assert_get_priorities_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_priorities_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let values = authenticated_client(base_url)
        .get_priorities()
        .await
        .map_err(|error| test_error(format!("get_priorities returned {error:?}")))?;

    assert_eq!(
        values
            .iter()
            .map(|value| (value.id.get(), value.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "major"), (2, "minor"), (3, "critical"), (4, "blocker")]
    );

    Ok(())
}

async fn assert_get_priorities_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_priorities().await).await
}

async fn assert_get_priorities_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_priorities().await).await
}

#[test]
fn get_time_entity_activities_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_time_entity_activities_200(&base_url).await?;
        assert_get_time_entity_activities_401(&base_url).await?;
        assert_get_time_entity_activities_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_time_entity_activities_200(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let values = authenticated_client(base_url)
        .get_time_entity_activities()
        .await
        .map_err(|error| test_error(format!("get_time_entity_activities returned {error:?}")))?;

    assert_eq!(
        values
            .iter()
            .map(|value| (value.id.get(), value.name.as_str(), value.is_default))
            .collect::<Vec<_>>(),
        // seederはenumerationのID衝突を避けるため、activityのIDに10000を加える。
        vec![
            (10001, "設計", true),
            (10002, "実装", false),
            (10003, "検証", false)
        ]
    );

    Ok(())
}

async fn assert_get_time_entity_activities_401(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_time_entity_activities().await).await
}

async fn assert_get_time_entity_activities_404(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_time_entity_activities().await).await
}

#[test]
fn get_trackers_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_trackers_200(&base_url).await?;
        assert_get_trackers_401(&base_url).await?;
        assert_get_trackers_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_trackers_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let values = authenticated_client(base_url)
        .get_trackers()
        .await
        .map_err(|error| test_error(format!("get_trackers returned {error:?}")))?;

    assert_eq!(
        values
            .iter()
            .map(|value| (value.id.get(), value.name.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "Bug"), (2, "Feature"), (3, "Support")]
    );

    Ok(())
}

async fn assert_get_trackers_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_trackers().await).await
}

async fn assert_get_trackers_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = not_found_client(base_url);
    expect_not_found(client.get_trackers().await).await
}
