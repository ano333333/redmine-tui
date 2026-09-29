use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::{EntityIdValue, IssueId, JournalId};

#[test]
fn update_journal_notes_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_update_journal_notes_replaces_notes(&base_url).await?;
        assert_update_journal_notes_with_empty_notes_deletes_a_journal_without_details(&base_url)
            .await?;
        assert_update_journal_notes_401(&base_url).await?;
        assert_update_journal_notes_404(&base_url).await?;
        Ok(())
    });
}

/// seedのIssue 3はJournal 1〜3を持ち、detailを持たないのはJournal 3だけである。
async fn fetch_issue_3_journal_notes(
    base_url: &str,
) -> Result<Vec<(u16, String)>, Box<dyn std::error::Error>> {
    Ok(authenticated_client(base_url)
        .get_issue(IssueId::new(3))
        .await
        .map_err(|error| test_error(format!("get_issue(3) returned {error:?}")))?
        .journals
        .into_iter()
        .map(|journal| (journal.id.get(), journal.notes))
        .collect())
}

async fn assert_update_journal_notes_replaces_notes(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    authenticated_client(base_url)
        .update_journal_notes(JournalId::new(3), "replaced notes")
        .await
        .map_err(|error| test_error(format!("update_journal_notes returned {error:?}")))?;

    assert_eq!(
        fetch_issue_3_journal_notes(base_url).await?,
        vec![
            (1, String::new()),
            (2, String::new()),
            (3, "replaced notes".to_string()),
        ]
    );

    Ok(())
}

async fn assert_update_journal_notes_with_empty_notes_deletes_a_journal_without_details(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    authenticated_client(base_url)
        .update_journal_notes(JournalId::new(3), "")
        .await
        .map_err(|error| test_error(format!("update_journal_notes returned {error:?}")))?;

    assert_eq!(
        fetch_issue_3_journal_notes(base_url).await?,
        vec![(1, String::new()), (2, String::new())]
    );

    Ok(())
}

async fn assert_update_journal_notes_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(
        client
            .update_journal_notes(JournalId::new(1), "notes")
            .await,
    )
    .await
}

async fn assert_update_journal_notes_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(
        client
            .update_journal_notes(JournalId::new(9999), "notes")
            .await,
    )
    .await
}
