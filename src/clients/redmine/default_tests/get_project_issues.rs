use std::num::NonZeroUsize;

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::{EntityIdValue, ProjectId};

#[test]
fn get_project_issues_requests_a_fixed_page_and_maps_its_metadata() {
    let mock_server = block_on(wiremock::MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/issues.json"))
            .and(query_param("project_id", "10"))
            .and(query_param("status_id", "*"))
            .and(query_param("sort", "id:desc"))
            .and(query_param("limit", "50"))
            .and(query_param("page", "2"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "issues":[{
                        "id":42,
                        "project":{"id":10},
                        "subject":"Fix login",
                        "description":"Login fails",
                        "status":{"id":3}
                    }],
                    "total_count":51,
                    "offset":50,
                    "limit":50
                }"#,
            ))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let result =
        block_on(client.get_project_issues(ProjectId::new(10), NonZeroUsize::new(2).unwrap()))
            .unwrap();

    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].id.get(), 42);
    assert_eq!(result.issues[0].project_id.get(), 10);
    assert_eq!(result.issues[0].subject, "Fix login");
    assert_eq!(result.issues[0].description, "Login fails");
    assert_eq!(result.issues[0].status_id.get(), 3);
    assert_eq!(result.total_count, 51);
    assert_eq!(result.offset, 50);
    assert_eq!(result.limit, 50);
}

#[test]
fn get_project_issues_maps_missing_and_null_descriptions_to_empty_strings() {
    let mock_server = block_on(wiremock::MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/issues.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "issues":[
                        {
                            "id":42,
                            "project":{"id":10},
                            "subject":"Missing description",
                            "status":{"id":3}
                        },
                        {
                            "id":41,
                            "project":{"id":10},
                            "subject":"Null description",
                            "description":null,
                            "status":{"id":3}
                        }
                    ],
                    "total_count":2,
                    "offset":0,
                    "limit":50
                }"#,
            ))
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let result = block_on(client.get_project_issues(ProjectId::new(10), NonZeroUsize::MIN))
        .expect("descriptions should be optional");

    assert_eq!(result.issues[0].description, "");
    assert_eq!(result.issues[1].description, "");
}

#[test]
fn get_project_issues_rejects_page_offset_overflow_before_sending_http_request() {
    let mock_server = block_on(wiremock::MockServer::start());
    block_on(
        Mock::given(method("GET"))
            .and(path("/issues.json"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let error = expect_error(
        block_on(client.get_project_issues(ProjectId::new(10), NonZeroUsize::MAX)),
        "page offset overflow succeeded",
    );

    assert!(matches!(error, RedmineClientError::Client { .. }));
}

#[test]
fn get_project_issues_rejects_inconsistent_page_metadata_or_project() {
    for body in [
        r#"{"issues":[],"total_count":0,"offset":0,"limit":49}"#,
        r#"{"issues":[],"total_count":0,"offset":51,"limit":50}"#,
        r#"{
            "issues":[{
                "id":42,
                "project":{"id":11},
                "subject":"Wrong project",
                "description":"",
                "status":{"id":3}
            }],
            "total_count":1,
            "offset":0,
            "limit":50
        }"#,
    ] {
        let mock_server = block_on(wiremock::MockServer::start());
        block_on(
            Mock::given(method("GET"))
                .and(path("/issues.json"))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&mock_server),
        );
        let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

        let error = expect_error(
            block_on(client.get_project_issues(ProjectId::new(10), NonZeroUsize::new(1).unwrap())),
            "inconsistent project issues response succeeded",
        );

        assert!(matches!(error, RedmineClientError::Client { .. }));
    }
}
