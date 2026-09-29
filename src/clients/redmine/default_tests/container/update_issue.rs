use chrono::NaiveDate;

use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::entities::IssueAggregate;
use crate::test_support::local_date;
use crate::vos::{EntityIdValue, IssueId, IssueStatusId, PriorityId, TrackerId, UserId};

#[test]
fn update_issue_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_update_issue_applies_every_property(&base_url).await?;
        assert_update_issue_clears_optional_properties(&base_url).await?;
        assert_update_issue_422(&base_url).await?;
        assert_update_issue_401(&base_url).await?;
        assert_update_issue_404(&base_url).await?;
        Ok(())
    });
}

async fn fetch_issue_1(base_url: &str) -> Result<IssueAggregate, Box<dyn std::error::Error>> {
    Ok(authenticated_client(base_url)
        .get_issue(IssueId::new(1))
        .await
        .map_err(|error| test_error(format!("get_issue(1) returned {error:?}")))?
        .aggregate)
}

async fn assert_update_issue_applies_every_property(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut issue = fetch_issue_1(base_url).await?;
    issue.issue.subject = "updated subject".to_string();
    issue.issue.description = "updated description".to_string();
    issue.issue.status_id = IssueStatusId::new(4);
    issue.tracker_id = TrackerId::new(2);
    issue.priority_id = PriorityId::new(2);
    issue.assigned_to_id = Some(UserId::new(1002));
    issue.start_date = Some(local_date(2026, 3, 1));
    issue.due_date = Some(local_date(2026, 3, 31));
    issue.done_ratio = 50;
    issue.estimated_hours = Some(2.5);

    authenticated_client(base_url)
        .update_issue(&issue)
        .await
        .map_err(|error| test_error(format!("update_issue returned {error:?}")))?;

    let updated = fetch_issue_1(base_url).await?;
    assert_eq!(updated.issue.subject, "updated subject");
    assert_eq!(updated.issue.description, "updated description");
    assert_eq!(updated.issue.status_id.get(), 4);
    assert_eq!(updated.tracker_id.get(), 2);
    assert_eq!(updated.priority_id.get(), 2);
    assert_eq!(updated.assigned_to_id.map(|id| id.get()), Some(1002));
    assert_eq!(
        updated.start_date.map(|date| date.date_naive()),
        NaiveDate::from_ymd_opt(2026, 3, 1)
    );
    assert_eq!(
        updated.due_date.map(|date| date.date_naive()),
        NaiveDate::from_ymd_opt(2026, 3, 31)
    );
    assert_eq!(updated.done_ratio, 50);
    assert_eq!(updated.estimated_hours, Some(2.5));

    Ok(())
}

async fn assert_update_issue_clears_optional_properties(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut issue = fetch_issue_1(base_url).await?;
    issue.target_version_id = None;
    issue.category_id = None;
    issue.start_date = None;
    issue.due_date = None;
    issue.estimated_hours = None;

    authenticated_client(base_url)
        .update_issue(&issue)
        .await
        .map_err(|error| test_error(format!("update_issue returned {error:?}")))?;

    let updated = fetch_issue_1(base_url).await?;
    assert_eq!(updated.target_version_id, None);
    assert_eq!(updated.category_id, None);
    assert_eq!(updated.start_date, None);
    assert_eq!(updated.due_date, None);
    assert_eq!(updated.estimated_hours, None);

    Ok(())
}

async fn assert_update_issue_422(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut issue = fetch_issue_1(base_url).await?;
    issue.issue.subject = String::new();

    match authenticated_client(base_url).update_issue(&issue).await {
        Err(RedmineClientError::UnprocessableEntity { context }) => {
            assert_eq!(context.status_code, 422);
            Ok(())
        }
        other => Err(test_error(format!(
            "expected UnprocessableEntity, got {other:?}"
        ))),
    }
}

async fn assert_update_issue_401(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let issue = fetch_issue_1(base_url).await?;
    expect_unauthorized(unauthorized_client(base_url).update_issue(&issue).await).await
}

async fn assert_update_issue_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut issue = fetch_issue_1(base_url).await?;
    issue.issue.id = IssueId::new(9999);
    expect_not_found(authenticated_client(base_url).update_issue(&issue).await).await
}
