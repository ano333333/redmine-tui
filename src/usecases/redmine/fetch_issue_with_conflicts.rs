use crate::clients::redmine::{RedmineClient, RedmineClientError};
use crate::entities::Issue;
use crate::vos::{EntityIdValue, IssueId, IssuePropertyDiff, JournalId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IssueProperty {
    Subject,
    AuthorId,
    CreatedOn,
    UpdatedOn,
    ProjectId,
    TrackerId,
    StatusId,
    PriorityId,
    AssignedToId,
    TargetVersionId,
    FixedVersion,
    StartDate,
    DueDate,
    DoneRatio,
    EstimatedHours,
    TotalSpentHours,
    ResolveWay,
    CategoryId,
    Description,
    ChildIds,
    JournalIds,
}

/// サーバーから最新の Issue を取得し、競合するローカルの property diff を返す。
///
/// 同じ property の diff は最初の `before` から最後の `after` へ畳み込み、差し引きで
/// 変更がないものは除外する。返す Issue はサーバーから取得したままの値であり、競合が
/// なければ [`apply_issue_property_diffs`] でローカルの変更を適用できる。
pub async fn fetch_issue_with_conflicts(
    client: &impl RedmineClient,
    id: IssueId,
    diffs: &[IssuePropertyDiff],
) -> Result<(Issue, Vec<IssuePropertyDiff>), RedmineClientError> {
    let issue = client.get_issue(id).await?;
    let conflicts = fold_property_diffs(diffs)
        .into_iter()
        .filter(|diff| conflicts_with_issue(&issue, diff))
        .collect();
    Ok((issue, conflicts))
}

/// 競合確認済みのローカル diff の `after` をサーバー由来の Issue に適用する。
///
/// 編集していない property には最新のサーバー値を残し、編集した property にはローカルの
/// 最終値を採用するため、Issue の現在値と diff の `before` は意図的に比較しない。
pub(crate) fn apply_issue_property_diffs(issue: &mut Issue, diffs: &[IssuePropertyDiff]) {
    for diff in &fold_property_diffs(diffs) {
        match diff {
            IssuePropertyDiff::Subject(diff) => issue.subject = diff.after.clone(),
            IssuePropertyDiff::AuthorId(diff) => issue.author_id = diff.after,
            IssuePropertyDiff::CreatedOn(diff) => issue.created_on = diff.after,
            IssuePropertyDiff::UpdatedOn(diff) => issue.updated_on = diff.after,
            IssuePropertyDiff::ProjectId(diff) => issue.project_id = diff.after,
            IssuePropertyDiff::TrackerId(diff) => issue.tracker_id = diff.after,
            IssuePropertyDiff::StatusId(diff) => issue.status_id = diff.after,
            IssuePropertyDiff::PriorityId(diff) => issue.priority_id = diff.after,
            IssuePropertyDiff::AssignedToId(diff) => issue.assigned_to_id = diff.after,
            IssuePropertyDiff::TargetVersionId(diff) => issue.target_version_id = diff.after,
            IssuePropertyDiff::StartDate(diff) => issue.start_date = diff.after,
            IssuePropertyDiff::DueDate(diff) => issue.due_date = diff.after,
            IssuePropertyDiff::DoneRatio(diff) => issue.done_ratio = diff.after,
            IssuePropertyDiff::EstimatedHours(diff) => issue.estimated_hours = diff.after,
            IssuePropertyDiff::TotalSpentHours(diff) => issue.total_spent_hours = diff.after,
            IssuePropertyDiff::CategoryId(diff) => issue.category_id = diff.after,
            IssuePropertyDiff::Description(diff) => issue.description = diff.after.clone(),
            IssuePropertyDiff::ChildIds(diff) => issue.child_ids = diff.after.clone(),
            IssuePropertyDiff::JournalIds(diff) => {
                issue.journal_ids = diff.after.iter().copied().map(JournalId::new).collect()
            }
            IssuePropertyDiff::FixedVersion(_) => {
                panic!("cannot apply FixedVersion diff: Issue has no fixed_version property")
            }
            IssuePropertyDiff::ResolveWay(_) => {
                panic!("cannot apply ResolveWay diff: Issue has no resolve_way property")
            }
        }
    }
}

fn fold_property_diffs(diffs: &[IssuePropertyDiff]) -> Vec<IssuePropertyDiff> {
    let mut folded = Vec::new();
    for diff in diffs {
        let property = property_of(diff);
        if let Some(existing) = folded
            .iter_mut()
            .find(|existing| property_of(existing) == property)
        {
            replace_after(existing, diff);
        } else {
            folded.push(diff.clone());
        }
    }
    folded.retain(|diff| !is_net_zero(diff));
    folded
}

fn property_of(diff: &IssuePropertyDiff) -> IssueProperty {
    match diff {
        IssuePropertyDiff::Subject(_) => IssueProperty::Subject,
        IssuePropertyDiff::AuthorId(_) => IssueProperty::AuthorId,
        IssuePropertyDiff::CreatedOn(_) => IssueProperty::CreatedOn,
        IssuePropertyDiff::UpdatedOn(_) => IssueProperty::UpdatedOn,
        IssuePropertyDiff::ProjectId(_) => IssueProperty::ProjectId,
        IssuePropertyDiff::TrackerId(_) => IssueProperty::TrackerId,
        IssuePropertyDiff::StatusId(_) => IssueProperty::StatusId,
        IssuePropertyDiff::PriorityId(_) => IssueProperty::PriorityId,
        IssuePropertyDiff::AssignedToId(_) => IssueProperty::AssignedToId,
        IssuePropertyDiff::TargetVersionId(_) => IssueProperty::TargetVersionId,
        IssuePropertyDiff::FixedVersion(_) => IssueProperty::FixedVersion,
        IssuePropertyDiff::StartDate(_) => IssueProperty::StartDate,
        IssuePropertyDiff::DueDate(_) => IssueProperty::DueDate,
        IssuePropertyDiff::DoneRatio(_) => IssueProperty::DoneRatio,
        IssuePropertyDiff::EstimatedHours(_) => IssueProperty::EstimatedHours,
        IssuePropertyDiff::TotalSpentHours(_) => IssueProperty::TotalSpentHours,
        IssuePropertyDiff::ResolveWay(_) => IssueProperty::ResolveWay,
        IssuePropertyDiff::CategoryId(_) => IssueProperty::CategoryId,
        IssuePropertyDiff::Description(_) => IssueProperty::Description,
        IssuePropertyDiff::ChildIds(_) => IssueProperty::ChildIds,
        IssuePropertyDiff::JournalIds(_) => IssueProperty::JournalIds,
    }
}

/// 2つのdiffが同じIssue propertyを対象にしているかを返す。
pub(crate) fn same_issue_property(left: &IssuePropertyDiff, right: &IssuePropertyDiff) -> bool {
    property_of(left) == property_of(right)
}

/// diffの`before`を、指定したサーバーIssueの現在値に置き換える。
pub(crate) fn with_server_value_as_before(
    issue: &Issue,
    diff: &IssuePropertyDiff,
) -> IssuePropertyDiff {
    let mut resolved = diff.clone();
    match &mut resolved {
        IssuePropertyDiff::Subject(diff) => diff.before = issue.subject.clone(),
        IssuePropertyDiff::AuthorId(diff) => diff.before = issue.author_id,
        IssuePropertyDiff::CreatedOn(diff) => diff.before = issue.created_on,
        IssuePropertyDiff::UpdatedOn(diff) => diff.before = issue.updated_on,
        IssuePropertyDiff::ProjectId(diff) => diff.before = issue.project_id,
        IssuePropertyDiff::TrackerId(diff) => diff.before = issue.tracker_id,
        IssuePropertyDiff::StatusId(diff) => diff.before = issue.status_id,
        IssuePropertyDiff::PriorityId(diff) => diff.before = issue.priority_id,
        IssuePropertyDiff::AssignedToId(diff) => diff.before = issue.assigned_to_id,
        IssuePropertyDiff::TargetVersionId(diff) => diff.before = issue.target_version_id,
        IssuePropertyDiff::StartDate(diff) => diff.before = issue.start_date,
        IssuePropertyDiff::DueDate(diff) => diff.before = issue.due_date,
        IssuePropertyDiff::DoneRatio(diff) => diff.before = issue.done_ratio,
        IssuePropertyDiff::EstimatedHours(diff) => diff.before = issue.estimated_hours,
        IssuePropertyDiff::TotalSpentHours(diff) => diff.before = issue.total_spent_hours,
        IssuePropertyDiff::CategoryId(diff) => diff.before = issue.category_id,
        IssuePropertyDiff::Description(diff) => diff.before = issue.description.clone(),
        IssuePropertyDiff::ChildIds(diff) => diff.before = issue.child_ids.clone(),
        IssuePropertyDiff::JournalIds(diff) => {
            diff.before = issue.journal_ids.iter().map(|id| id.get()).collect()
        }
        IssuePropertyDiff::FixedVersion(_) => {
            panic!("サーバーIssueにfixed_version propertyがないため解決できません")
        }
        IssuePropertyDiff::ResolveWay(_) => {
            panic!("サーバーIssueにresolve_way propertyがないため解決できません")
        }
    }
    resolved
}

/// diffの`after`を、指定したサーバーIssueの現在値に置き換える。
pub(crate) fn with_server_value_as_after(
    issue: &Issue,
    diff: &IssuePropertyDiff,
) -> IssuePropertyDiff {
    let mut resolved = diff.clone();
    match &mut resolved {
        IssuePropertyDiff::Subject(diff) => diff.after = issue.subject.clone(),
        IssuePropertyDiff::AuthorId(diff) => diff.after = issue.author_id,
        IssuePropertyDiff::CreatedOn(diff) => diff.after = issue.created_on,
        IssuePropertyDiff::UpdatedOn(diff) => diff.after = issue.updated_on,
        IssuePropertyDiff::ProjectId(diff) => diff.after = issue.project_id,
        IssuePropertyDiff::TrackerId(diff) => diff.after = issue.tracker_id,
        IssuePropertyDiff::StatusId(diff) => diff.after = issue.status_id,
        IssuePropertyDiff::PriorityId(diff) => diff.after = issue.priority_id,
        IssuePropertyDiff::AssignedToId(diff) => diff.after = issue.assigned_to_id,
        IssuePropertyDiff::TargetVersionId(diff) => diff.after = issue.target_version_id,
        IssuePropertyDiff::StartDate(diff) => diff.after = issue.start_date,
        IssuePropertyDiff::DueDate(diff) => diff.after = issue.due_date,
        IssuePropertyDiff::DoneRatio(diff) => diff.after = issue.done_ratio,
        IssuePropertyDiff::EstimatedHours(diff) => diff.after = issue.estimated_hours,
        IssuePropertyDiff::TotalSpentHours(diff) => diff.after = issue.total_spent_hours,
        IssuePropertyDiff::CategoryId(diff) => diff.after = issue.category_id,
        IssuePropertyDiff::Description(diff) => diff.after = issue.description.clone(),
        IssuePropertyDiff::ChildIds(diff) => diff.after = issue.child_ids.clone(),
        IssuePropertyDiff::JournalIds(diff) => {
            diff.after = issue.journal_ids.iter().map(|id| id.get()).collect()
        }
        IssuePropertyDiff::FixedVersion(_) => {
            panic!("サーバーIssueにfixed_version propertyがないため解決できません")
        }
        IssuePropertyDiff::ResolveWay(_) => {
            panic!("サーバーIssueにresolve_way propertyがないため解決できません")
        }
    }
    resolved
}

macro_rules! match_same_diff {
    ($left:expr, $right:expr, |$a:ident, $b:ident| $body:expr) => {
        match ($left, $right) {
            (IssuePropertyDiff::Subject($a), IssuePropertyDiff::Subject($b)) => $body,
            (IssuePropertyDiff::AuthorId($a), IssuePropertyDiff::AuthorId($b)) => $body,
            (IssuePropertyDiff::CreatedOn($a), IssuePropertyDiff::CreatedOn($b)) => $body,
            (IssuePropertyDiff::UpdatedOn($a), IssuePropertyDiff::UpdatedOn($b)) => $body,
            (IssuePropertyDiff::ProjectId($a), IssuePropertyDiff::ProjectId($b)) => $body,
            (IssuePropertyDiff::TrackerId($a), IssuePropertyDiff::TrackerId($b)) => $body,
            (IssuePropertyDiff::StatusId($a), IssuePropertyDiff::StatusId($b)) => $body,
            (IssuePropertyDiff::PriorityId($a), IssuePropertyDiff::PriorityId($b)) => $body,
            (IssuePropertyDiff::AssignedToId($a), IssuePropertyDiff::AssignedToId($b)) => $body,
            (IssuePropertyDiff::TargetVersionId($a), IssuePropertyDiff::TargetVersionId($b)) => {
                $body
            }
            (IssuePropertyDiff::FixedVersion($a), IssuePropertyDiff::FixedVersion($b)) => $body,
            (IssuePropertyDiff::StartDate($a), IssuePropertyDiff::StartDate($b)) => $body,
            (IssuePropertyDiff::DueDate($a), IssuePropertyDiff::DueDate($b)) => $body,
            (IssuePropertyDiff::DoneRatio($a), IssuePropertyDiff::DoneRatio($b)) => $body,
            (IssuePropertyDiff::EstimatedHours($a), IssuePropertyDiff::EstimatedHours($b)) => $body,
            (IssuePropertyDiff::TotalSpentHours($a), IssuePropertyDiff::TotalSpentHours($b)) => {
                $body
            }
            (IssuePropertyDiff::ResolveWay($a), IssuePropertyDiff::ResolveWay($b)) => $body,
            (IssuePropertyDiff::CategoryId($a), IssuePropertyDiff::CategoryId($b)) => $body,
            (IssuePropertyDiff::Description($a), IssuePropertyDiff::Description($b)) => $body,
            (IssuePropertyDiff::ChildIds($a), IssuePropertyDiff::ChildIds($b)) => $body,
            (IssuePropertyDiff::JournalIds($a), IssuePropertyDiff::JournalIds($b)) => $body,
            _ => unreachable!("property identity must match diff variants"),
        }
    };
}

// 同じ macro で Copy 型と Clone のみの型を扱うため、代入方法を clone に統一する。
#[allow(clippy::clone_on_copy)]
fn replace_after(existing: &mut IssuePropertyDiff, latest: &IssuePropertyDiff) {
    match_same_diff!(existing, latest, |existing, latest| {
        existing.after = latest.after.clone()
    });
}

fn is_net_zero(diff: &IssuePropertyDiff) -> bool {
    match_same_diff!(diff, diff, |left, right| left.before == right.after)
}

fn conflicts_with_issue(issue: &Issue, diff: &IssuePropertyDiff) -> bool {
    macro_rules! conflict {
        ($server:expr, $diff:expr) => {{
            let server = &$server;
            server != &$diff.before && server != &$diff.after
        }};
    }

    match diff {
        IssuePropertyDiff::Subject(diff) => conflict!(issue.subject, diff),
        IssuePropertyDiff::AuthorId(diff) => conflict!(issue.author_id, diff),
        IssuePropertyDiff::CreatedOn(diff) => conflict!(issue.created_on, diff),
        IssuePropertyDiff::UpdatedOn(diff) => conflict!(issue.updated_on, diff),
        IssuePropertyDiff::ProjectId(diff) => conflict!(issue.project_id, diff),
        IssuePropertyDiff::TrackerId(diff) => conflict!(issue.tracker_id, diff),
        IssuePropertyDiff::StatusId(diff) => conflict!(issue.status_id, diff),
        IssuePropertyDiff::PriorityId(diff) => conflict!(issue.priority_id, diff),
        IssuePropertyDiff::AssignedToId(diff) => conflict!(issue.assigned_to_id, diff),
        IssuePropertyDiff::TargetVersionId(diff) => conflict!(issue.target_version_id, diff),
        IssuePropertyDiff::StartDate(diff) => conflict!(issue.start_date, diff),
        IssuePropertyDiff::DueDate(diff) => conflict!(issue.due_date, diff),
        IssuePropertyDiff::DoneRatio(diff) => conflict!(issue.done_ratio, diff),
        IssuePropertyDiff::EstimatedHours(diff) => conflict!(issue.estimated_hours, diff),
        IssuePropertyDiff::TotalSpentHours(diff) => conflict!(issue.total_spent_hours, diff),
        IssuePropertyDiff::CategoryId(diff) => conflict!(issue.category_id, diff),
        IssuePropertyDiff::Description(diff) => conflict!(issue.description, diff),
        IssuePropertyDiff::ChildIds(diff) => conflict!(issue.child_ids, diff),
        IssuePropertyDiff::JournalIds(diff) => {
            let server = issue
                .journal_ids
                .iter()
                .map(|id| id.get())
                .collect::<Vec<_>>();
            conflict!(server, diff)
        }
        IssuePropertyDiff::FixedVersion(_) => {
            panic!("cannot compare FixedVersion diff: Issue has no fixed_version property")
        }
        IssuePropertyDiff::ResolveWay(_) => {
            panic!("cannot compare ResolveWay diff: Issue has no resolve_way property")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::clients::redmine::{RedmineClient, RedmineClientError};
    use crate::entities::{
        Category, Issue, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity,
        Tracker, User,
    };
    use crate::test_support::{local_datetime, sample_issue};
    use crate::vos::issue_property_diff::{
        IssueDescriptionDiff, IssueDueDateDiff, IssueStatusIdDiff,
    };
    use crate::vos::{IssueId, IssuePropertyDiff, IssueStatusId};

    use super::{apply_issue_property_diffs, fetch_issue_with_conflicts};

    #[tokio::test]
    async fn fetches_requested_issue_and_returns_it() {
        let server_issue = issue("server subject", "server description", 1);
        let client = StubClient::new(server_issue.clone());

        let (actual, conflicts) = fetch_issue_with_conflicts(&client, IssueId::new(42), &[])
            .await
            .unwrap();

        assert_eq!(client.requested_ids(), vec![IssueId::new(42)]);
        assert_eq!(actual.id, server_issue.id);
        assert_eq!(actual.subject, server_issue.subject);
        assert!(conflicts.is_empty());
    }

    #[tokio::test]
    async fn folds_same_property_and_discards_net_zero_change() {
        let client = StubClient::new(issue("subject", "original", 1));
        let diffs = vec![
            description_diff("original", "middle"),
            description_diff("middle", "original"),
        ];

        let (_, conflicts) = fetch_issue_with_conflicts(&client, 1.into(), &diffs)
            .await
            .unwrap();

        assert!(conflicts.is_empty());
    }

    #[tokio::test]
    async fn returns_only_folded_diffs_that_conflict_with_server() {
        let client = StubClient::new(issue("subject", "server edit", 2));
        let diffs = vec![
            description_diff("original", "middle"),
            IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: IssueStatusId::new(1),
                after: IssueStatusId::new(2),
            }),
            description_diff("middle", "local edit"),
        ];

        let (_, conflicts) = fetch_issue_with_conflicts(&client, 1.into(), &diffs)
            .await
            .unwrap();

        assert_eq!(conflicts, vec![description_diff("original", "local edit")]);
    }

    #[tokio::test]
    async fn server_equal_to_before_or_after_is_not_a_conflict() {
        let before_client = StubClient::new(issue("subject", "before", 1));
        let after_client = StubClient::new(issue("subject", "after", 1));
        let diffs = vec![description_diff("before", "after")];

        let (_, before_conflicts) = fetch_issue_with_conflicts(&before_client, 1.into(), &diffs)
            .await
            .unwrap();
        let (_, after_conflicts) = fetch_issue_with_conflicts(&after_client, 1.into(), &diffs)
            .await
            .unwrap();

        assert!(before_conflicts.is_empty());
        assert!(after_conflicts.is_empty());
    }

    #[test]
    fn applies_local_after_values_over_different_server_values_in_order() {
        let mut server_issue = issue("server subject", "server edit", 3);
        let diffs = vec![
            description_diff("original", "middle"),
            description_diff("middle", "local edit"),
            IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: IssueStatusId::new(1),
                after: IssueStatusId::new(2),
            }),
            IssuePropertyDiff::DueDate(IssueDueDateDiff {
                before: None,
                after: Some(local_datetime("2026-08-23T00:00:00+09:00")),
            }),
        ];

        apply_issue_property_diffs(&mut server_issue, &diffs);

        assert_eq!(server_issue.subject, "server subject");
        assert_eq!(server_issue.description, "local edit");
        assert_eq!(server_issue.status_id, IssueStatusId::new(2));
        assert_eq!(
            server_issue.due_date,
            Some(local_datetime("2026-08-23T00:00:00+09:00"))
        );
    }

    #[test]
    fn applying_net_zero_diff_preserves_the_server_value() {
        let mut server_issue = issue("subject", "server edit", 1);
        let diffs = vec![
            description_diff("original", "middle"),
            description_diff("middle", "original"),
        ];

        apply_issue_property_diffs(&mut server_issue, &diffs);

        assert_eq!(server_issue.description, "server edit");
    }

    fn description_diff(before: &str, after: &str) -> IssuePropertyDiff {
        IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: before.to_string(),
            after: after.to_string(),
        })
    }

    fn issue(subject: &str, description: &str, status: u16) -> Issue {
        let mut issue = sample_issue(1, subject, IssueStatusId::new(status), None, None, None, 0);
        issue.description = description.to_string();
        issue
    }

    struct StubClient {
        issue: Issue,
        requested_ids: Mutex<Vec<IssueId>>,
    }

    impl StubClient {
        fn new(issue: Issue) -> Self {
            Self {
                issue,
                requested_ids: Mutex::new(Vec::new()),
            }
        }

        fn requested_ids(&self) -> Vec<IssueId> {
            self.requested_ids.lock().unwrap().clone()
        }
    }

    impl RedmineClient for StubClient {
        async fn get_issue(&self, id: IssueId) -> Result<Issue, RedmineClientError> {
            self.requested_ids.lock().unwrap().push(id);
            Ok(self.issue.clone())
        }

        async fn update_issue(&self, _: &Issue) -> Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn get_categories(&self) -> Result<Vec<Category>, RedmineClientError> {
            unreachable!()
        }

        async fn get_issue_statuses(&self) -> Result<Vec<IssueStatus>, RedmineClientError> {
            unreachable!()
        }

        async fn get_priorities(&self) -> Result<Vec<Priority>, RedmineClientError> {
            unreachable!()
        }

        async fn get_projects(&self) -> Result<Vec<Project>, RedmineClientError> {
            unreachable!()
        }

        async fn get_target_versions(&self) -> Result<Vec<TargetVersion>, RedmineClientError> {
            unreachable!()
        }

        async fn get_time_entity_activities(
            &self,
        ) -> Result<Vec<TimeEntityActivity>, RedmineClientError> {
            unreachable!()
        }

        async fn get_trackers(&self) -> Result<Vec<Tracker>, RedmineClientError> {
            unreachable!()
        }

        async fn get_users(&self) -> Result<Vec<User>, RedmineClientError> {
            unreachable!()
        }
    }
}
