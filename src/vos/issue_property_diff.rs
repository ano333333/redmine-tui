use chrono::{DateTime, Local};

use crate::vos::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct IssueSubjectDiff {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueAuthorIdDiff {
    pub before: UserId,
    pub after: UserId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueCreatedOnDiff {
    pub before: DateTime<Local>,
    pub after: DateTime<Local>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueUpdatedOnDiff {
    pub before: DateTime<Local>,
    pub after: DateTime<Local>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueProjectIdDiff {
    pub before: ProjectId,
    pub after: ProjectId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueTrackerIdDiff {
    pub before: TrackerId,
    pub after: TrackerId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueStatusIdDiff {
    pub before: IssueStatusId,
    pub after: IssueStatusId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssuePriorityIdDiff {
    pub before: PriorityId,
    pub after: PriorityId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueAssignedToIdDiff {
    pub before: Option<UserId>,
    pub after: Option<UserId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueFixedVersionDiff {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueTargetVersionIdDiff {
    pub before: Option<TargetVersionId>,
    pub after: Option<TargetVersionId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueStartDateDiff {
    pub before: Option<DateTime<Local>>,
    pub after: Option<DateTime<Local>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueDueDateDiff {
    pub before: Option<DateTime<Local>>,
    pub after: Option<DateTime<Local>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueDoneRatioDiff {
    pub before: u16,
    pub after: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueEstimatedHoursDiff {
    pub before: Option<f64>,
    pub after: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueTotalSpentHoursDiff {
    pub before: Option<f64>,
    pub after: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueResolveWayDiff {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueCategoryIdDiff {
    pub before: Option<CategoryId>,
    pub after: Option<CategoryId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueDescriptionDiff {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueChildIdsDiff {
    pub before: Vec<IssueId>,
    pub after: Vec<IssueId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IssuePropertyDiff {
    Subject(IssueSubjectDiff),
    AuthorId(IssueAuthorIdDiff),
    CreatedOn(IssueCreatedOnDiff),
    UpdatedOn(IssueUpdatedOnDiff),
    ProjectId(IssueProjectIdDiff),
    TrackerId(IssueTrackerIdDiff),
    StatusId(IssueStatusIdDiff),
    PriorityId(IssuePriorityIdDiff),
    AssignedToId(IssueAssignedToIdDiff),
    TargetVersionId(IssueTargetVersionIdDiff),
    FixedVersion(IssueFixedVersionDiff),
    StartDate(IssueStartDateDiff),
    DueDate(IssueDueDateDiff),
    DoneRatio(IssueDoneRatioDiff),
    EstimatedHours(IssueEstimatedHoursDiff),
    TotalSpentHours(IssueTotalSpentHoursDiff),
    ResolveWay(IssueResolveWayDiff),
    CategoryId(IssueCategoryIdDiff),
    Description(IssueDescriptionDiff),
    ChildIds(IssueChildIdsDiff),
}

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
}

/// 同じ property の diff を最初の `before` から最後の `after` へ畳み込み、差し引きで変更が
/// ないものを除いて返す。結果は各 property が最初に現れた順に並ぶ。
pub fn fold_property_diffs(diffs: &[IssuePropertyDiff]) -> Vec<IssuePropertyDiff> {
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
    }
}

/// 2つのdiffが同じIssue propertyを対象にしているかを返す。
pub fn same_issue_property(left: &IssuePropertyDiff, right: &IssuePropertyDiff) -> bool {
    property_of(left) == property_of(right)
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

#[cfg(test)]
mod tests {
    use super::{
        IssueDescriptionDiff, IssuePropertyDiff, IssueStatusIdDiff, fold_property_diffs,
        same_issue_property,
    };
    use crate::vos::IssueStatusId;

    fn description_diff(before: &str, after: &str) -> IssuePropertyDiff {
        IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: before.to_string(),
            after: after.to_string(),
        })
    }

    fn status_diff(before: u16, after: u16) -> IssuePropertyDiff {
        IssuePropertyDiff::StatusId(IssueStatusIdDiff {
            before: IssueStatusId::new(before),
            after: IssueStatusId::new(after),
        })
    }

    #[test]
    fn folding_keeps_the_first_before_and_the_last_after_of_each_property() {
        let diffs = vec![
            description_diff("original", "middle"),
            status_diff(1, 2),
            description_diff("middle", "latest"),
        ];

        let folded = fold_property_diffs(&diffs);

        assert_eq!(
            folded,
            vec![description_diff("original", "latest"), status_diff(1, 2)]
        );
    }

    #[test]
    fn folding_discards_a_property_edited_back_to_its_original_value() {
        let diffs = vec![
            description_diff("original", "middle"),
            status_diff(1, 2),
            description_diff("middle", "original"),
        ];

        let folded = fold_property_diffs(&diffs);

        assert_eq!(folded, vec![status_diff(1, 2)]);
    }

    #[test]
    fn same_issue_property_compares_only_the_property_kind() {
        assert!(same_issue_property(
            &description_diff("a", "b"),
            &description_diff("c", "d")
        ));
        assert!(!same_issue_property(
            &description_diff("a", "b"),
            &status_diff(1, 2)
        ));
    }
}
