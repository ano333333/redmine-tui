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
        r#"{"versions":[{"id":7,"name":"v1.0","project":{"id":10,"name":"Redmine TUI"}}]}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let versions = block_on(client.get_target_versions()).unwrap();

    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].id.get(), 7);
    assert_eq!(versions[0].name, "v1.0");
    assert_eq!(versions[0].project_id.get(), 10);
}
