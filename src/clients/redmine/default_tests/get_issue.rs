use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use super::{block_on, expect_error, http_error_for_url, mount_get};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient, RedmineClientError};
use crate::vos::{EntityIdValue, IssueId, JournalDetail, JournalDetailAttr, JournalId, JournalKey};

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
                "estimated_hours": 8.0,
                "total_spent_hours": 2.5,
                "category": {"id": 4},
                "description": "Login fails with valid credentials",
                "children": [{"id": 43}],
                "journals": [
                    {"id": 500, "user": {"id": 1001, "name": "Alice"},
                     "updated_on": "2026-08-18T13:30:00Z", "notes": "note",
                     "details": [
                       {"property": "attr", "name": "status_id", "old_value": "1", "new_value": "2"},
                       {"property": "attr", "name": "subject", "old_value": "old", "new_value": "new"}
                     ]},
                    {"id": 501, "user": {"id": 1002, "name": "Bob"},
                     "updated_on": "2026-08-18T14:00:00+00:00", "details": []}
                ]
            }
        }"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let (issue, journals) = block_on(client.get_issue(IssueId::new(42))).unwrap();

    assert_eq!(issue.issue.id.get(), 42);
    assert_eq!(issue.issue.subject, "Fix login");
    assert_eq!(issue.author_id.get(), 1000);
    assert_eq!(issue.issue.project_id.get(), 10);
    assert_eq!(issue.tracker_id.get(), 2);
    assert_eq!(issue.issue.status_id.get(), 1);
    assert_eq!(issue.priority_id.get(), 5);
    assert_eq!(issue.assigned_to_id.unwrap().get(), 1001);
    assert_eq!(issue.target_version_id.unwrap().get(), 7);
    assert_eq!(issue.done_ratio, 30);
    assert_eq!(issue.estimated_hours, Some(8));
    assert_eq!(issue.total_spent_hours, Some(2.5));
    assert_eq!(issue.category_id.unwrap().get(), 4);
    assert_eq!(
        issue.issue.description,
        "Login fails with valid credentials"
    );
    assert_eq!(journals.len(), 2);
    assert_eq!(
        (journals[0].id, journals[1].id),
        (JournalId::new(500), JournalId::new(501))
    );
    assert_eq!(
        (journals[0].user.as_str(), journals[1].user.as_str()),
        ("Alice", "Bob")
    );
    assert_eq!(journals[0].notes, "note");
    assert_eq!(journals[1].notes, "");
    assert_eq!(
        journals[0].updated_on.timestamp(),
        chrono::DateTime::parse_from_rfc3339("2026-08-18T13:30:00Z")
            .unwrap()
            .timestamp()
    );
    let [
        JournalDetail::Attr(JournalDetailAttr::StatusId { old, new }),
        JournalDetail::Attr(JournalDetailAttr::Subject {
            old: old_subject,
            new: new_subject,
        }),
    ] = journals[0].details.as_slice()
    else {
        panic!("details must retain wire order")
    };
    assert_eq!(
        (*old, *new),
        (
            crate::vos::IssueStatusId::new(1),
            crate::vos::IssueStatusId::new(2)
        )
    );
    assert_eq!((old_subject.as_str(), new_subject.as_str()), ("old", "new"));
    assert_eq!(issue.child_ids[0].get(), 43);
    assert_eq!(
        issue.journal_keys,
        vec![
            JournalKey::Remote(JournalId::new(500)),
            JournalKey::Remote(JournalId::new(501)),
        ]
    );
}

#[test]
fn get_issue_rejects_a_malformed_journal_detail() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        200,
        r#"{"issue": {
            "id": 42, "subject": "subject", "author": {"id": 1},
            "created_on": "2026-08-18T12:00:00Z", "updated_on": "2026-08-18T13:00:00Z",
            "project": {"id": 1}, "tracker": {"id": 1}, "status": {"id": 1},
            "priority": {"id": 1}, "done_ratio": 0,
            "journals": [{"id": 500, "user": {"id": 1, "name": "Alice"},
                "updated_on": "2026-08-18T13:30:00Z", "details": [
                    {"property": "attr", "name": "status_id", "old_value": "invalid", "new_value": "2"}
                ]}]
        }}"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    assert!(matches!(
        block_on(client.get_issue(IssueId::new(42))),
        Err(RedmineClientError::Client { .. })
    ));
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
    let (issue, journals) = authenticated_client(base_url)
        .get_issue(IssueId::new(1))
        .await
        .map_err(|error| test_error(format!("get_issue(1) returned {error:?}")))?;

    assert_eq!(issue.issue.id.get(), 1);
    assert_eq!(issue.issue.subject, "issue1");
    assert_eq!(issue.author_id.get(), 1001);
    assert_eq!(issue.issue.project_id.get(), 1);
    assert_eq!(
        issue.journal_keys,
        journals
            .iter()
            .map(|journal| JournalKey::Remote(journal.id))
            .collect::<Vec<_>>()
    );

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
