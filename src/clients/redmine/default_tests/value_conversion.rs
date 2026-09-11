use super::super::value_conversion::{parse_datetime, parse_optional_date};
use super::expect_error;
use crate::clients::redmine::RedmineClientError;

#[test]
fn invalid_redmine_datetime_maps_to_client_error_with_reason() {
    let actual_error = expect_error(parse_datetime("not-a-date"), "datetime parse succeeded");

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("failed to parse Redmine datetime 'not-a-date'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn invalid_redmine_date_maps_to_client_error_with_reason() {
    let actual_error = expect_error(
        parse_optional_date(Some("not-a-date".to_string())),
        "date parse succeeded",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("failed to parse Redmine date 'not-a-date'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
