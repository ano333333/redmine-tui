use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use super::{block_on, expect_error, http_error_for_url, mount_get};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::{EntityIdValue, IssueId, JournalDetail, JournalDetailAttr, JournalId};

#[test]
fn get_issue_maps_success_response() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        200,
        r#"{
            "issue": {
                "id": 42,
                "subject": "Fix login",
                "author": {"id": 1000},
                "created_on": "2026-08-18T12:00:00Z",
                "updated_on": "2026-08-18T13:00:00Z",
                "project": {"id": 10},
                "tracker": {"id": 2},
                "status": {"id": 1},
                "priority": {"id": 5},
                "assigned_to": {"id": 1001},
                "fixed_version": {"id": 7},
                "start_date": "2026-08-19",
                "due_date": "2026-08-31",
                "done_ratio": 30,
                "estimated_hours": 1.5,
                "total_spent_hours": 2.5,
                "category": {"id": 4},
                "description": "Login fails with valid credentials",
                "children": [{"id": 43}],
                "journals": [
                    {
                        "id": 500,
                        "user": {"id": 1000, "name": "Taro Yamada"},
                        "updated_on": "2026-08-18T14:00:00Z",
                        "notes": "first journal"
                    }
                ]
            }
        }"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let issue = block_on(client.get_issue(IssueId::new(42))).unwrap();

    let aggregate = &issue.aggregate;

    assert_eq!(aggregate.issue.id.get(), 42);
    assert_eq!(aggregate.issue.subject, "Fix login");
    assert_eq!(aggregate.author_id.get(), 1000);
    assert_eq!(aggregate.issue.project_id.get(), 10);
    assert_eq!(aggregate.tracker_id.get(), 2);
    assert_eq!(aggregate.issue.status_id.get(), 1);
    assert_eq!(aggregate.priority_id.get(), 5);
    assert_eq!(aggregate.assigned_to_id.unwrap().get(), 1001);
    assert_eq!(aggregate.target_version_id.unwrap().get(), 7);
    assert_eq!(aggregate.done_ratio, 30);
    assert_eq!(aggregate.estimated_hours, Some(1.5));
    assert_eq!(aggregate.total_spent_hours, Some(2.5));
    assert_eq!(aggregate.category_id.unwrap().get(), 4);
    assert_eq!(
        aggregate.issue.description,
        "Login fails with valid credentials"
    );
    assert_eq!(aggregate.child_ids[0].get(), 43);
    assert_eq!(issue.journals.len(), 1);
}

#[test]
fn get_issue_maps_unauthorized_with_response_context() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        401,
        r#"{"error":"failed"}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(
        block_on(client.get_issue(IssueId::new(42))),
        "get_issue succeeded",
    );

    match actual_error {
        RedmineClientError::Unauthorized { context } => {
            assert_eq!(
                context,
                http_error_for_url(
                    401,
                    &format!(
                        "{}/issues/42.json?include=children,journals",
                        mock_server.uri()
                    ),
                )
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn get_issue_maps_not_found_with_response_context() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        404,
        r#"{"error":"failed"}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let actual_error = expect_error(
        block_on(client.get_issue(IssueId::new(42))),
        "get_issue succeeded",
    );

    match actual_error {
        RedmineClientError::NotFound { context } => {
            assert_eq!(
                context,
                http_error_for_url(
                    404,
                    &format!(
                        "{}/issues/42.json?include=children,journals",
                        mock_server.uri()
                    ),
                )
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
#[ignore = "requires Docker and pulls/starts Redmine via testcontainers"]
fn get_issue_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_issue_200(&base_url).await?;
        assert_get_issue_401(&base_url).await?;
        assert_get_issue_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_get_issue_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fetched = authenticated_client(base_url)
        .get_issue(IssueId::new(1))
        .await
        .map_err(|error| test_error(format!("get_issue(1) returned {error:?}")))?;

    assert_eq!(fetched.aggregate.issue.id.get(), 1);
    assert_eq!(fetched.aggregate.issue.subject, "issue1");
    assert_eq!(fetched.aggregate.author_id.get(), 1001);
    assert_eq!(fetched.aggregate.issue.project_id.get(), 1);

    Ok(())
}

async fn assert_get_issue_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.get_issue(IssueId::new(1)).await).await
}

async fn assert_get_issue_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(client.get_issue(IssueId::new(9999)).await).await
}
#[test]
fn get_issue_builds_fetched_issue_with_converted_journals() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        200,
        r#"{
            "issue": {
                "id": 42,
                "subject": "Fix login",
                "author": {"id": 1000},
                "created_on": "2026-08-18T12:00:00Z",
                "updated_on": "2026-08-18T13:00:00Z",
                "project": {"id": 10},
                "tracker": {"id": 2},
                "status": {"id": 1},
                "priority": {"id": 5},
                "done_ratio": 30,
                "journals": [
                    {
                        "id": 500,
                        "user": {"id": 1000, "name": "Taro Yamada"},
                        "updated_on": "2026-08-18T14:00:00Z",
                        "notes": "first journal",
                        "details": [
                            {"property": "attr", "name": "status_id", "old_value": "1", "new_value": "2"},
                            {"property": "attr", "name": "subject", "old_value": "old subject", "new_value": "new subject"}
                        ]
                    },
                    {
                        "id": 501,
                        "user": {"id": 1001, "name": "Hanako Suzuki"},
                        "updated_on": "2026-08-19T15:00:00Z",
                        "notes": "second journal",
                        "details": [
                            {"property": "attr", "name": "done_ratio", "old_value": "0", "new_value": "100"}
                        ]
                    }
                ]
            }
        }"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let fetched = block_on(client.get_issue(IssueId::new(42))).unwrap();

    assert_eq!(fetched.aggregate.issue.id.get(), 42);
    assert_eq!(fetched.aggregate.issue.subject, "Fix login");
    assert_eq!(fetched.journals.len(), 2);

    let first = &fetched.journals[0];
    assert_eq!(first.id, JournalId::new(500));
    assert_eq!(first.issue_id, IssueId::new(42));
    assert_eq!(first.user, "Taro Yamada");
    assert_eq!(first.notes, "first journal");
    assert_eq!(first.details.len(), 2);
    match &first.details[0] {
        JournalDetail::Attr(JournalDetailAttr::StatusId { old, new }) => {
            assert_eq!(*old, crate::vos::IssueStatusId::new(1));
            assert_eq!(*new, crate::vos::IssueStatusId::new(2));
        }
        _ => panic!("unexpected first detail"),
    }
    match &first.details[1] {
        JournalDetail::Attr(JournalDetailAttr::Subject { old, new }) => {
            assert_eq!(old, "old subject");
            assert_eq!(new, "new subject");
        }
        _ => panic!("unexpected second detail"),
    }

    let second = &fetched.journals[1];
    assert_eq!(second.id, JournalId::new(501));
    assert_eq!(second.issue_id, IssueId::new(42));
    assert_eq!(second.user, "Hanako Suzuki");
    assert_eq!(second.notes, "second journal");
    assert_eq!(second.details.len(), 1);
    match &second.details[0] {
        JournalDetail::Attr(JournalDetailAttr::DoneRatio { old, new }) => {
            assert_eq!(*old, 0);
            assert_eq!(*new, 100);
        }
        _ => panic!("unexpected detail"),
    }
}

#[test]
fn get_issue_skips_unsupported_journal_detail_properties() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        200,
        r#"{
            "issue": {
                "id": 42,
                "subject": "Fix login",
                "author": {"id": 1000},
                "created_on": "2026-08-18T12:00:00Z",
                "updated_on": "2026-08-18T13:00:00Z",
                "project": {"id": 10},
                "tracker": {"id": 2},
                "status": {"id": 1},
                "priority": {"id": 5},
                "done_ratio": 30,
                "journals": [
                    {
                        "id": 500,
                        "user": {"id": 1000, "name": "Taro Yamada"},
                        "updated_on": "2026-08-18T14:00:00Z",
                        "notes": "first journal",
                        "details": [
                            {"property": "attr", "name": "status_id", "old_value": "1", "new_value": "2"},
                            {"property": "cf_1", "name": "custom1", "old_value": null, "new_value": "custom value"},
                            {"property": "attachment", "name": "file.txt"},
                            {"property": "relation", "name": "43", "relation_type": "child"}
                        ]
                    }
                ]
            }
        }"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let fetched = block_on(client.get_issue(IssueId::new(42))).unwrap();

    assert_eq!(fetched.journals.len(), 1);
    assert_eq!(fetched.journals[0].id, JournalId::new(500));
    assert_eq!(fetched.journals[0].issue_id, IssueId::new(42));
    assert_eq!(fetched.journals[0].details.len(), 1);
    match &fetched.journals[0].details[0] {
        JournalDetail::Attr(JournalDetailAttr::StatusId { old, new }) => {
            assert_eq!(*old, crate::vos::IssueStatusId::new(1));
            assert_eq!(*new, crate::vos::IssueStatusId::new(2));
        }
        _ => panic!("unsupported details were not skipped"),
    }
}
