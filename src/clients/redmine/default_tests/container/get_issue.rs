use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::test_support::{SAMPLE_MARKDOWN, local_date, local_datetime};
use crate::vos::{EntityIdValue, IssueId, JournalDetail, JournalDetailAttr};

#[test]
fn get_issue_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_issue_200(&base_url).await?;
        assert_get_issue_401(&base_url).await?;
        assert_get_issue_404(&base_url).await?;
        Ok(())
    });
}

/// seedのIssue 3は子Issue 1・2と、status変更・期日変更・notesのJournal 1〜3を持つ。
async fn assert_get_issue_200(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fetched = authenticated_client(base_url)
        .get_issue(IssueId::new(3))
        .await
        .map_err(|error| test_error(format!("get_issue(3) returned {error:?}")))?;

    let issue = fetched.aggregate;
    assert_eq!(issue.issue.id.get(), 3);
    assert_eq!(issue.issue.project_id.get(), 1);
    assert_eq!(
        issue.issue.subject,
        "issue1(長ああああああああああああああああああああああああああああああああああああいタイトル)"
    );
    assert_eq!(issue.issue.description, SAMPLE_MARKDOWN);
    assert_eq!(issue.issue.status_id.get(), 3);
    assert_eq!(issue.author_id.get(), 1001);
    assert_eq!(issue.created_on, local_datetime("2026-02-04T00:00:00Z"));
    assert_eq!(issue.updated_on, local_datetime("2026-02-16T00:00:00Z"));
    assert_eq!(issue.tracker_id.get(), 3);
    assert_eq!(issue.priority_id.get(), 1);
    assert_eq!(issue.assigned_to_id.map(|id| id.get()), Some(1001));
    assert_eq!(issue.target_version_id, None);
    assert_eq!(issue.start_date, Some(local_date(2026, 2, 16)));
    assert_eq!(issue.due_date, Some(local_date(2026, 2, 17)));
    assert_eq!(issue.done_ratio, 0);
    assert_eq!(issue.estimated_hours, None);
    assert_eq!(issue.total_spent_hours, Some(0.0));
    assert_eq!(issue.category_id.map(|id| id.get()), Some(1));
    assert_eq!(
        issue
            .child_ids
            .iter()
            .map(|id| id.get())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    assert_eq!(
        fetched
            .journals
            .iter()
            .map(|journal| (
                journal.id.get(),
                journal.issue_id.get(),
                journal.user.as_str(),
                journal.updated_on,
                journal.notes.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                1,
                3,
                "user1 Fixture",
                Some(local_datetime("2026-02-10T00:00:00Z")),
                "",
            ),
            (
                2,
                3,
                "user1 Fixture",
                Some(local_datetime("2026-02-16T00:00:00Z")),
                "",
            ),
            (
                3,
                3,
                "user1 Fixture",
                Some(local_datetime("2026-02-16T00:00:00Z")),
                SAMPLE_MARKDOWN,
            ),
        ]
    );
    assert!(matches!(
        fetched.journals[0].details.as_slice(),
        [JournalDetail::Attr(JournalDetailAttr::StatusId { old, new })]
            if old.get() == 1 && new.get() == 2
    ));
    assert!(matches!(
        fetched.journals[1].details.as_slice(),
        [JournalDetail::Attr(JournalDetailAttr::DueDate { old, new })]
            if *old == Some(local_date(2026, 2, 16)) && *new == Some(local_date(2026, 2, 17))
    ));
    assert!(fetched.journals[2].details.is_empty());

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
