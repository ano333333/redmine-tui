use chrono::NaiveDate;

use super::integration_support::{
    authenticated_client, expect_not_found, expect_unauthorized, run_contract, test_error,
    unauthorized_client,
};
use crate::clients::redmine::{IssueUpdate, RedmineClient, RedmineClientError};
use crate::entities::IssueAggregate;
use crate::test_support::local_date;
use crate::vos::{EntityIdValue, IssueId, IssueStatusId, PriorityId, TrackerId, UserId};

#[test]
fn update_issue_contract_against_redmine_container() {
    run_contract(|base_url| async move {
        assert_update_issue_keeps_unsent_properties(&base_url).await?;
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

async fn update_issue_1(
    base_url: &str,
    update: IssueUpdate,
) -> Result<(), Box<dyn std::error::Error>> {
    authenticated_client(base_url)
        .update_issue(IssueId::new(1), &update)
        .await
        .map_err(|error| test_error(format!("update_issue returned {error:?}")))
}

/// seedのIssue 1は、status 3、担当者1001、対象バージョン1、カテゴリー1、期日2025-12-19を持つ。
async fn assert_update_issue_keeps_unsent_properties(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    update_issue_1(
        base_url,
        IssueUpdate {
            subject: Some("only subject".to_string()),
            ..IssueUpdate::default()
        },
    )
    .await?;

    let updated = fetch_issue_1(base_url).await?;
    assert_eq!(updated.issue.subject, "only subject");
    assert_eq!(updated.issue.status_id.get(), 3);
    assert_eq!(updated.assigned_to_id.map(|id| id.get()), Some(1001));
    assert_eq!(updated.target_version_id.map(|id| id.get()), Some(1));
    assert_eq!(updated.category_id.map(|id| id.get()), Some(1));
    assert_eq!(
        updated.due_date.map(|date| date.date_naive()),
        NaiveDate::from_ymd_opt(2025, 12, 19)
    );

    Ok(())
}

async fn assert_update_issue_applies_every_property(
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    update_issue_1(
        base_url,
        IssueUpdate {
            subject: Some("updated subject".to_string()),
            description: Some("updated description".to_string()),
            status_id: Some(IssueStatusId::new(4)),
            tracker_id: Some(TrackerId::new(2)),
            priority_id: Some(PriorityId::new(2)),
            assigned_to_id: Some(Some(UserId::new(1002))),
            start_date: Some(Some(local_date(2026, 3, 1))),
            due_date: Some(Some(local_date(2026, 3, 31))),
            done_ratio: Some(50),
            estimated_hours: Some(Some(2.5)),
            ..IssueUpdate::default()
        },
    )
    .await?;

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
    update_issue_1(
        base_url,
        IssueUpdate {
            assigned_to_id: Some(None),
            target_version_id: Some(None),
            category_id: Some(None),
            start_date: Some(None),
            due_date: Some(None),
            estimated_hours: Some(None),
            ..IssueUpdate::default()
        },
    )
    .await?;

    let updated = fetch_issue_1(base_url).await?;
    assert_eq!(updated.assigned_to_id, None);
    assert_eq!(updated.target_version_id, None);
    assert_eq!(updated.category_id, None);
    assert_eq!(updated.start_date, None);
    assert_eq!(updated.due_date, None);
    assert_eq!(updated.estimated_hours, None);

    Ok(())
}

async fn assert_update_issue_422(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let update = IssueUpdate {
        subject: Some(String::new()),
        ..IssueUpdate::default()
    };

    match authenticated_client(base_url)
        .update_issue(IssueId::new(1), &update)
        .await
    {
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
    let update = IssueUpdate {
        subject: Some("unauthorized".to_string()),
        ..IssueUpdate::default()
    };
    expect_unauthorized(
        unauthorized_client(base_url)
            .update_issue(IssueId::new(1), &update)
            .await,
    )
    .await
}

async fn assert_update_issue_404(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let update = IssueUpdate {
        subject: Some("missing".to_string()),
        ..IssueUpdate::default()
    };
    expect_not_found(
        authenticated_client(base_url)
            .update_issue(IssueId::new(9999), &update)
            .await,
    )
    .await
}
