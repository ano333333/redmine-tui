use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::{EntityIdValue, IssueId};

/// seedのJournalはID 1〜3のため、AUTO_INCREMENTから次に作成されるIDは4になる。
const CREATED_JOURNAL_ID: u16 = 4;

#[test]
fn update_issue_notes_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_update_issue_notes_creates_a_journal(&base_url).await?;
        assert_update_issue_notes_401(&base_url).await?;
        assert_update_issue_notes_404(&base_url).await?;
        Ok(())
    });
}

async fn assert_update_issue_notes_creates_a_journal(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    client
        .update_issue_notes(IssueId::new(1), "added notes")
        .await
        .map_err(|error| test_error(format!("update_issue_notes returned {error:?}")))?;

    let journals = client
        .get_issue(IssueId::new(1))
        .await
        .map_err(|error| test_error(format!("get_issue(1) returned {error:?}")))?
        .journals;
    assert_eq!(
        journals
            .iter()
            .map(|journal| (journal.id.get(), journal.notes.as_str()))
            .collect::<Vec<_>>(),
        vec![(CREATED_JOURNAL_ID, "added notes")]
    );

    Ok(())
}

async fn assert_update_issue_notes_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(client.update_issue_notes(IssueId::new(1), "notes").await).await
}

async fn assert_update_issue_notes_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(client.update_issue_notes(IssueId::new(9999), "notes").await).await
}
