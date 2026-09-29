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
