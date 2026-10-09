use chrono::{DateTime, Local};

use crate::entities::IssueAggregate;
use crate::vos::{
    CategoryId, IssueId, IssuePropertyDiff, IssueStatusId, PriorityId, ProjectId, TargetVersionId,
    TrackerId, UserId,
};

/// サーバーから取得した基準値に、未保存の編集を反映して見せる表示用の参照。
///
/// 各属性は、その属性の最後のdiffの`after`、diffがなければ基準値を返す。保存時に送る値を
/// 求める`IssueAggregate::with_property_diffs`と同じ規則であり、値を複製せずにStoreの寿命で
/// 参照を返すために別に実装している。
#[derive(Clone, Copy, Debug)]
pub struct IssueView<'a> {
    base: &'a IssueAggregate,
    diffs: &'a [IssuePropertyDiff],
}

macro_rules! last_after {
    ($self:ident, $variant:ident, $base:expr) => {
        $self
            .diffs
            .iter()
            .rev()
            .find_map(|diff| match diff {
                IssuePropertyDiff::$variant(diff) => Some(&diff.after),
                _ => None,
            })
            .unwrap_or(&$base)
    };
}

impl<'a> IssueView<'a> {
    /// `diffs`は編集した順に並んでいる前提で、後のdiffを優先する。
    pub fn new(base: &'a IssueAggregate, diffs: &'a [IssuePropertyDiff]) -> Self {
        Self { base, diffs }
    }

    pub fn id(&self) -> IssueId {
        self.base.issue.id
    }

    pub fn subject(&self) -> &'a str {
        let base = self.base;
        last_after!(self, Subject, base.issue.subject)
    }

    pub fn description(&self) -> &'a str {
        let base = self.base;
        last_after!(self, Description, base.issue.description)
    }

    pub fn project_id(&self) -> ProjectId {
        *last_after!(self, ProjectId, self.base.issue.project_id)
    }

    pub fn status_id(&self) -> IssueStatusId {
        *last_after!(self, StatusId, self.base.issue.status_id)
    }

    pub fn author_id(&self) -> UserId {
        self.base.author_id
    }

    pub fn created_on(&self) -> DateTime<Local> {
        self.base.created_on
    }

    pub fn updated_on(&self) -> DateTime<Local> {
        self.base.updated_on
    }

    pub fn tracker_id(&self) -> TrackerId {
        *last_after!(self, TrackerId, self.base.tracker_id)
    }

    pub fn priority_id(&self) -> PriorityId {
        *last_after!(self, PriorityId, self.base.priority_id)
    }

    pub fn assigned_to_id(&self) -> Option<UserId> {
        *last_after!(self, AssignedToId, self.base.assigned_to_id)
    }

    pub fn target_version_id(&self) -> Option<TargetVersionId> {
        *last_after!(self, TargetVersionId, self.base.target_version_id)
    }

    pub fn start_date(&self) -> Option<DateTime<Local>> {
        *last_after!(self, StartDate, self.base.start_date)
    }

    pub fn due_date(&self) -> Option<DateTime<Local>> {
        *last_after!(self, DueDate, self.base.due_date)
    }

    pub fn done_ratio(&self) -> u16 {
        *last_after!(self, DoneRatio, self.base.done_ratio)
    }

    pub fn estimated_hours(&self) -> Option<f64> {
        *last_after!(self, EstimatedHours, self.base.estimated_hours)
    }

    pub fn total_spent_hours(&self) -> Option<f64> {
        self.base.total_spent_hours
    }

    pub fn category_id(&self) -> Option<CategoryId> {
        *last_after!(self, CategoryId, self.base.category_id)
    }
}

#[cfg(test)]
mod tests {
    use super::IssueView;
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::issue_property_diff::{
        IssueAssignedToIdDiff, IssueDescriptionDiff, IssueStatusIdDiff,
    };
    use crate::vos::{IssuePropertyDiff, IssueStatusId, UserId};

    fn description_diff(before: &str, after: &str) -> IssuePropertyDiff {
        IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: before.to_string(),
            after: after.to_string(),
        })
    }

    #[test]
    fn returns_the_last_after_of_each_edited_property() {
        let mut base = sample_issue_aggregate(1, "subject", 1.into(), Some(7), None, None, 0);
        base.issue.description = "fetched".to_string();
        let diffs = vec![
            description_diff("fetched", "first"),
            IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: IssueStatusId::new(1),
                after: IssueStatusId::new(2),
            }),
            description_diff("first", "second"),
            IssuePropertyDiff::AssignedToId(IssueAssignedToIdDiff {
                before: Some(UserId::new(7)),
                after: None,
            }),
        ];

        let view = IssueView::new(&base, &diffs);

        assert_eq!(view.description(), "second");
        assert_eq!(view.status_id(), IssueStatusId::new(2));
        assert_eq!(view.assigned_to_id(), None);
    }

    #[test]
    fn returns_fetched_values_for_unedited_properties() {
        let base = sample_issue_aggregate(1, "subject", 1.into(), Some(7), None, None, 30);
        let diffs = vec![description_diff("body", "edited")];

        let view = IssueView::new(&base, &diffs);

        assert_eq!(view.subject(), "subject");
        assert_eq!(view.status_id(), IssueStatusId::new(1));
        assert_eq!(view.assigned_to_id(), Some(UserId::new(7)));
        assert_eq!(view.done_ratio(), 30);
    }
}
