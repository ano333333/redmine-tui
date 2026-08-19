use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, not_found_client, run_contract,
    test_error, unauthorized_client,
};
use super::{block_on, mount_get};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient};
use crate::vos::EntityIdValue;

#[test]
fn get_issue_statuses_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issue_statuses.json",
        200,
        r#"{"issue_statuses":[{"id":1,"name":"New","is_closed":false}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let statuses = block_on(client.get_issue_statuses()).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id.get(), 1);
    assert_eq!(statuses[0].name, "New");
    assert!(!statuses[0].is_closed);
}

#[test]
fn get_priorities_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/enumerations/issue_priorities.json",
        200,
        r#"{"issue_priorities":[{"id":5,"name":"High"}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let priorities = block_on(client.get_priorities()).unwrap();

    assert_eq!(priorities.len(), 1);
    assert_eq!(priorities[0].id.get(), 5);
    assert_eq!(priorities[0].name, "High");
}

#[test]
fn get_time_entity_activities_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/enumerations/time_entry_activities.json",
        200,
        r#"{"time_entry_activities":[{"id":9,"name":"Development","is_default":true}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let activities = block_on(client.get_time_entity_activities()).unwrap();

    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].id.get(), 9);
    assert_eq!(activities[0].name, "Development");
    assert!(activities[0].is_default);
}

#[test]
fn get_trackers_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/trackers.json",
        200,
        r#"{"trackers":[{"id":2,"name":"Bug"}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let trackers = block_on(client.get_trackers()).unwrap();

    assert_eq!(trackers.len(), 1);
    assert_eq!(trackers[0].id.get(), 2);
    assert_eq!(trackers[0].name, "Bug");
}

#[test]
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_issue_statuses_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_issue_statuses_200(&base_url).await?;
        assert_get_issue_statuses_401(&base_url).await?;
        assert_get_issue_statuses_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_issue_statuses_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let statuses = authenticated_client(base_url)
        .get_issue_statuses()
        .await
        .map_err(|error| test_error(format!("get_issue_statuses returned {error:?}")))?;

    if statuses.is_empty() {
        return Err(test_error("get_issue_statuses returned no statuses"));
    }
    if !statuses.iter().any(|status| status.name == "新規(new)") {
        return Err(test_error(format!(
            "get_issue_statuses did not include 新規(new); got {}",
            names(statuses.iter().map(|status| status.name.as_str()))
        )));
    }

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
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_priorities_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_priorities_200(&base_url).await?;
        assert_get_priorities_401(&base_url).await?;
        assert_get_priorities_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_priorities_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let priorities = authenticated_client(base_url)
        .get_priorities()
        .await
        .map_err(|error| test_error(format!("get_priorities returned {error:?}")))?;

    if priorities.is_empty() {
        return Err(test_error("get_priorities returned no priorities"));
    }
    if !priorities.iter().any(|priority| priority.name == "major") {
        return Err(test_error(format!(
            "get_priorities did not include major; got {}",
            names(priorities.iter().map(|priority| priority.name.as_str()))
        )));
    }

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
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
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
    let activities = authenticated_client(base_url)
        .get_time_entity_activities()
        .await
        .map_err(|error| test_error(format!("get_time_entity_activities returned {error:?}")))?;

    if activities.is_empty() {
        return Err(test_error(
            "get_time_entity_activities returned no activities",
        ));
    }
    if !activities.iter().any(|activity| activity.name == "設計") {
        return Err(test_error(format!(
            "get_time_entity_activities did not include 設計; got {}",
            names(activities.iter().map(|activity| activity.name.as_str()))
        )));
    }

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
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_trackers_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_trackers_200(&base_url).await?;
        assert_get_trackers_401(&base_url).await?;
        assert_get_trackers_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_trackers_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let trackers = authenticated_client(base_url)
        .get_trackers()
        .await
        .map_err(|error| test_error(format!("get_trackers returned {error:?}")))?;

    if trackers.is_empty() {
        return Err(test_error("get_trackers returned no trackers"));
    }
    if !trackers
        .iter()
        .any(|tracker| matches!(tracker.name.as_str(), "Bug" | "Feature" | "Support"))
    {
        return Err(test_error(format!(
            "get_trackers did not include Bug, Feature, or Support; got {}",
            names(trackers.iter().map(|tracker| tracker.name.as_str()))
        )));
    }

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

fn names<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values.collect::<Vec<_>>().join(", ")
}
