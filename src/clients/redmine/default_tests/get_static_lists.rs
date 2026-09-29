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
