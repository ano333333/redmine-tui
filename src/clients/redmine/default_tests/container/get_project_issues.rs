use std::num::NonZeroUsize;

use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::RedmineClient;
use crate::vos::{EntityIdValue, ProjectId};

const FIRST_PAGE: NonZeroUsize = NonZeroUsize::MIN;

#[test]
fn get_project_issues_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_get_project_issues_first_page(&base_url).await?;
        assert_get_project_issues_page_past_the_end(&base_url).await?;
        assert_get_project_issues_401(&base_url).await?;
        assert_get_project_issues_404(&base_url).await?;
        Ok(())
    });
}

/// seedのproject 1にはIssue 1〜3があり、Issue 2は完了statusである。
async fn assert_get_project_issues_first_page(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let page = authenticated_client(base_url)
        .get_project_issues(ProjectId::new(1), FIRST_PAGE)
        .await
        .map_err(|error| test_error(format!("get_project_issues page 1 returned {error:?}")))?;

    assert_eq!(
        page.issues
            .iter()
            .map(|issue| (issue.id.get(), issue.status_id.get()))
            .collect::<Vec<_>>(),
        vec![(4, 3), (3, 3), (2, 5), (1, 3)]
    );
    assert_eq!(page.total_count, 4);
    assert_eq!(page.offset, 0);
    assert_eq!(page.limit, 50);

    Ok(())
}

async fn assert_get_project_issues_page_past_the_end(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let page = authenticated_client(base_url)
        .get_project_issues(ProjectId::new(1), NonZeroUsize::new(2).unwrap())
        .await
        .map_err(|error| test_error(format!("get_project_issues page 2 returned {error:?}")))?;

    assert!(page.issues.is_empty());
    assert_eq!(page.total_count, 4);
    assert_eq!(page.offset, 50);

    Ok(())
}

async fn assert_get_project_issues_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = unauthorized_client(base_url);
    expect_unauthorized(
        client
            .get_project_issues(ProjectId::new(1), FIRST_PAGE)
            .await,
    )
    .await
}

async fn assert_get_project_issues_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = authenticated_client(base_url);
    expect_not_found(
        client
            .get_project_issues(ProjectId::new(9999), FIRST_PAGE)
            .await,
    )
    .await
}
