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
