use std::num::NonZeroUsize;

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{block_on, expect_error};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::ProjectId;

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
