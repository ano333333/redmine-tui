use super::super::journal_conversion::{RedmineJournal, RedmineJournalUser, convert_journal};
use super::expect_error;
use crate::clients::redmine::RedmineClientError;
use crate::vos::{IssueId, JournalId};

fn sample_journal() -> RedmineJournal {
    RedmineJournal {
        id: 7,
        user: RedmineJournalUser {
            id: 3,
            name: "Taro Yamada".to_string(),
        },
        updated_on: Some("2026-01-02T03:04:05+00:00".to_string()),
        notes: "This is a journal".to_string(),
        details: Vec::new(),
    }
}

#[test]
fn valid_redmine_journal_is_converted_with_requested_issue_id() {
    let issue_id = IssueId::new(99);
    let expected_updated_on = chrono::DateTime::parse_from_rfc3339("2026-01-02T03:04:05+00:00")
        .unwrap()
        .with_timezone(&chrono::Local);

    let actual_journal =
        convert_journal(issue_id, sample_journal()).expect("journal conversion failed");

    assert_eq!(actual_journal.id, JournalId::new(7));
    assert_eq!(actual_journal.issue_id, issue_id);
    assert_eq!(actual_journal.user, "Taro Yamada");
    assert_eq!(actual_journal.updated_on, Some(expected_updated_on));
    assert_eq!(actual_journal.notes, "This is a journal");
    assert!(actual_journal.details.is_empty());
}

#[test]
fn null_redmine_journal_updated_on_is_converted_to_none() {
    let mut journal = sample_journal();
    journal.updated_on = None;

    let actual_journal =
        convert_journal(IssueId::new(99), journal).expect("journal conversion failed");

    assert_eq!(actual_journal.updated_on, None);
}

#[test]
fn invalid_redmine_journal_updated_on_maps_to_client_error_with_reason() {
    let mut journal = sample_journal();
    journal.updated_on = Some("not-a-datetime".to_string());

    let actual_error = expect_error(
        convert_journal(IssueId::new(99), journal),
        "invalid journal conversion succeeded",
    );

    match actual_error {
        RedmineClientError::Client { reason } => {
            assert!(
                reason.contains("failed to parse Redmine datetime 'not-a-datetime'"),
                "unexpected reason: {reason}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
