use tokio::runtime::Builder as TokioRuntimeBuilder;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::clients::redmine::{RedmineClientError, RedmineHttpError};

mod create_journal;
mod error_mapping;
mod get_categories;
mod get_issue;
mod get_project_issues;
mod get_projects;
mod get_static_lists;
mod get_target_versions;
mod get_users;
mod integration_support;
mod journal_detail_conversion;
mod journal_detail_value_conversion;
mod update_issue;
mod update_journal_notes;

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn expect_error<T>(result: Result<T, RedmineClientError>, message: &str) -> RedmineClientError {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

fn mount_get(mock_server: &MockServer, path_value: &str, status_code: u16, body: &str) {
    block_on(
        Mock::given(method("GET"))
            .and(path(path_value))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .respond_with(ResponseTemplate::new(status_code).set_body_string(body))
            .expect(1)
            .mount(mock_server),
    );
}

fn mount_get_paginated(mock_server: &MockServer, path_value: &str, status_code: u16, body: &str) {
    block_on(
        Mock::given(method("GET"))
            .and(path(path_value))
            .and(query_param("limit", "100"))
            .and(query_param("offset", "0"))
            .and(header("X-Redmine-API-Key", "secret-token"))
            .respond_with(ResponseTemplate::new(status_code).set_body_string(body))
            .expect(1)
            .mount(mock_server),
    );
}

fn http_error(status_code: u16, base_url: &str) -> RedmineHttpError {
    http_error_for_url(
        status_code,
        &format!("{base_url}/users.json?limit=100&offset=0"),
    )
}

fn http_error_for_url(status_code: u16, url: &str) -> RedmineHttpError {
    RedmineHttpError {
        method: "GET".to_string(),
        url: url.to_string(),
        status_code,
        response_body: r#"{"error":"failed"}"#.to_string(),
    }
}
