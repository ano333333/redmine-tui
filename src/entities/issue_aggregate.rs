use chrono::{DateTime, Local};

use crate::entities::{Issue, Journal};
use crate::vos::issue_property_diff::fold_property_diffs;
use crate::vos::{
    CategoryId, IssueId, IssuePropertyDiff, PriorityId, TargetVersionId, TrackerId, UserId,
};

/// 詳細用のIssueエンティティ。
#[derive(Debug, Clone)]
pub struct IssueAggregate {
    /// 一覧表示に必要な情報を保持する軽量なIssueエンティティ。
    pub issue: Issue,
    pub author_id: UserId,
    pub created_on: DateTime<Local>,
    pub updated_on: DateTime<Local>,
    pub tracker_id: TrackerId,
    pub priority_id: PriorityId,
    pub assigned_to_id: Option<UserId>,
    pub target_version_id: Option<TargetVersionId>,
    pub start_date: Option<DateTime<Local>>,
    pub due_date: Option<DateTime<Local>>,
    pub done_ratio: u16,
    pub estimated_hours: Option<f64>,
    pub total_spent_hours: Option<f64>,
    pub category_id: Option<CategoryId>,
    pub parent_id: Option<IssueId>,
    /// 取得順のJournal。
    pub journals: Vec<Journal>,
}

impl IssueAggregate {
    /// 属性ごとに集約したdiffの`after`を、このIssueの値に適用した値を返す。
    ///
    /// diffのない属性と、元の値に戻して差分がなくなった属性は、このIssueの値のまま残る。
    /// このIssueの値が集約後の`before`と`after`のどちらとも異なる属性があれば、何も適用せず、
    /// それらの集約後のdiffを`Err`で返す。
    pub fn with_property_diffs(
        &self,
        diffs: &[IssuePropertyDiff],
    ) -> Result<IssueAggregate, Vec<IssuePropertyDiff>> {
        let folded = fold_property_diffs(diffs);
        let conflicts: Vec<IssuePropertyDiff> = folded
            .iter()
            .filter(|diff| self.conflicts_with(diff))
            .cloned()
            .collect();
        if !conflicts.is_empty() {
            return Err(conflicts);
        }
        let mut applied = self.clone();
        for diff in &folded {
            applied.set_after(diff);
        }
        Ok(applied)
    }

    fn set_after(&mut self, diff: &IssuePropertyDiff) {
        match diff {
            IssuePropertyDiff::Subject(diff) => self.issue.subject = diff.after.clone(),
            IssuePropertyDiff::ProjectId(diff) => self.issue.project_id = diff.after,
            IssuePropertyDiff::TrackerId(diff) => self.tracker_id = diff.after,
            IssuePropertyDiff::StatusId(diff) => self.issue.status_id = diff.after,
            IssuePropertyDiff::PriorityId(diff) => self.priority_id = diff.after,
            IssuePropertyDiff::AssignedToId(diff) => self.assigned_to_id = diff.after,
            IssuePropertyDiff::TargetVersionId(diff) => self.target_version_id = diff.after,
            IssuePropertyDiff::StartDate(diff) => self.start_date = diff.after,
            IssuePropertyDiff::DueDate(diff) => self.due_date = diff.after,
            IssuePropertyDiff::DoneRatio(diff) => self.done_ratio = diff.after,
            IssuePropertyDiff::EstimatedHours(diff) => self.estimated_hours = diff.after,
            IssuePropertyDiff::CategoryId(diff) => self.category_id = diff.after,
            IssuePropertyDiff::Description(diff) => self.issue.description = diff.after.clone(),
        }
    }

    fn conflicts_with(&self, diff: &IssuePropertyDiff) -> bool {
        macro_rules! conflict {
            ($server:expr, $diff:expr) => {{
                let server = &$server;
                server != &$diff.before && server != &$diff.after
            }};
        }

        match diff {
            IssuePropertyDiff::Subject(diff) => conflict!(self.issue.subject, diff),
            IssuePropertyDiff::ProjectId(diff) => conflict!(self.issue.project_id, diff),
            IssuePropertyDiff::TrackerId(diff) => conflict!(self.tracker_id, diff),
            IssuePropertyDiff::StatusId(diff) => conflict!(self.issue.status_id, diff),
            IssuePropertyDiff::PriorityId(diff) => conflict!(self.priority_id, diff),
            IssuePropertyDiff::AssignedToId(diff) => conflict!(self.assigned_to_id, diff),
            IssuePropertyDiff::TargetVersionId(diff) => conflict!(self.target_version_id, diff),
            IssuePropertyDiff::StartDate(diff) => conflict!(self.start_date, diff),
            IssuePropertyDiff::DueDate(diff) => conflict!(self.due_date, diff),
            IssuePropertyDiff::DoneRatio(diff) => conflict!(self.done_ratio, diff),
            IssuePropertyDiff::EstimatedHours(diff) => conflict!(self.estimated_hours, diff),
            IssuePropertyDiff::CategoryId(diff) => conflict!(self.category_id, diff),
            IssuePropertyDiff::Description(diff) => conflict!(self.issue.description, diff),
        }
    }

    /// diffの`before`を、このIssueの現在値に置き換えたdiffを返す。
    pub fn with_value_as_before(&self, diff: &IssuePropertyDiff) -> IssuePropertyDiff {
        let mut resolved = diff.clone();
        match &mut resolved {
            IssuePropertyDiff::Subject(diff) => diff.before = self.issue.subject.clone(),
            IssuePropertyDiff::ProjectId(diff) => diff.before = self.issue.project_id,
            IssuePropertyDiff::TrackerId(diff) => diff.before = self.tracker_id,
            IssuePropertyDiff::StatusId(diff) => diff.before = self.issue.status_id,
            IssuePropertyDiff::PriorityId(diff) => diff.before = self.priority_id,
            IssuePropertyDiff::AssignedToId(diff) => diff.before = self.assigned_to_id,
            IssuePropertyDiff::TargetVersionId(diff) => diff.before = self.target_version_id,
            IssuePropertyDiff::StartDate(diff) => diff.before = self.start_date,
            IssuePropertyDiff::DueDate(diff) => diff.before = self.due_date,
            IssuePropertyDiff::DoneRatio(diff) => diff.before = self.done_ratio,
            IssuePropertyDiff::EstimatedHours(diff) => diff.before = self.estimated_hours,
            IssuePropertyDiff::CategoryId(diff) => diff.before = self.category_id,
            IssuePropertyDiff::Description(diff) => diff.before = self.issue.description.clone(),
        }
        resolved
    }

    /// diffの`after`を、このIssueの現在値に置き換えたdiffを返す。
    pub fn with_value_as_after(&self, diff: &IssuePropertyDiff) -> IssuePropertyDiff {
        let mut resolved = diff.clone();
        match &mut resolved {
            IssuePropertyDiff::Subject(diff) => diff.after = self.issue.subject.clone(),
            IssuePropertyDiff::ProjectId(diff) => diff.after = self.issue.project_id,
            IssuePropertyDiff::TrackerId(diff) => diff.after = self.tracker_id,
            IssuePropertyDiff::StatusId(diff) => diff.after = self.issue.status_id,
            IssuePropertyDiff::PriorityId(diff) => diff.after = self.priority_id,
            IssuePropertyDiff::AssignedToId(diff) => diff.after = self.assigned_to_id,
            IssuePropertyDiff::TargetVersionId(diff) => diff.after = self.target_version_id,
            IssuePropertyDiff::StartDate(diff) => diff.after = self.start_date,
            IssuePropertyDiff::DueDate(diff) => diff.after = self.due_date,
            IssuePropertyDiff::DoneRatio(diff) => diff.after = self.done_ratio,
            IssuePropertyDiff::EstimatedHours(diff) => diff.after = self.estimated_hours,
            IssuePropertyDiff::CategoryId(diff) => diff.after = self.category_id,
            IssuePropertyDiff::Description(diff) => diff.after = self.issue.description.clone(),
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use crate::entities::IssueAggregate;
    use crate::test_support::{local_datetime, sample_issue_aggregate};
    use crate::vos::issue_property_diff::{
        IssueDescriptionDiff, IssueDueDateDiff, IssueStatusIdDiff,
    };
    use crate::vos::{IssuePropertyDiff, IssueStatusId};

    fn description_diff(before: &str, after: &str) -> IssuePropertyDiff {
        IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: before.to_string(),
            after: after.to_string(),
        })
    }

    fn issue(subject: &str, description: &str, status: u16) -> IssueAggregate {
        let mut issue =
            sample_issue_aggregate(1, subject, IssueStatusId::new(status), None, None, None, 0);
        issue.issue.description = description.to_string();
        issue
    }

    #[test]
    fn applies_the_last_after_of_each_edited_property_over_current_values() {
        let server_issue = issue("server subject", "original", 1);
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

        let applied = server_issue
            .with_property_diffs(&diffs)
            .expect("server values match before");

        assert_eq!(applied.issue.subject, "server subject");
        assert_eq!(applied.issue.description, "local edit");
        assert_eq!(applied.issue.status_id, IssueStatusId::new(2));
        assert_eq!(
            applied.due_date,
            Some(local_datetime("2026-08-23T00:00:00+09:00"))
        );
    }

    #[test]
    fn a_property_edited_back_keeps_the_current_value_even_if_it_changed_on_the_server() {
        let server_issue = issue("subject", "server edit", 1);
        let diffs = vec![
            description_diff("original", "middle"),
            description_diff("middle", "original"),
        ];

        let applied = server_issue
            .with_property_diffs(&diffs)
            .expect("net-zero edits cannot conflict");

        assert_eq!(applied.issue.description, "server edit");
    }

    #[test]
    fn a_current_value_equal_to_before_or_after_is_not_a_conflict() {
        let diffs = vec![description_diff("before", "after")];

        let from_before = issue("subject", "before", 1).with_property_diffs(&diffs);
        let from_after = issue("subject", "after", 1).with_property_diffs(&diffs);

        assert_eq!(
            from_before
                .expect("before is not a conflict")
                .issue
                .description,
            "after"
        );
        assert_eq!(
            from_after
                .expect("after is not a conflict")
                .issue
                .description,
            "after"
        );
    }

    #[test]
    fn returns_only_the_folded_diffs_that_conflict_with_current_values() {
        let server_issue = issue("subject", "server edit", 2);
        let diffs = vec![
            description_diff("original", "middle"),
            IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: IssueStatusId::new(1),
                after: IssueStatusId::new(2),
            }),
            description_diff("middle", "local edit"),
        ];

        let conflicts = server_issue
            .with_property_diffs(&diffs)
            .expect_err("description changed on the server");

        assert_eq!(conflicts, vec![description_diff("original", "local edit")]);
    }

    #[test]
    fn replaces_before_or_after_with_the_current_value() {
        let issue = issue("subject", "server", 1);
        let diff = description_diff("before", "after");

        assert_eq!(
            issue.with_value_as_before(&diff),
            description_diff("server", "after")
        );
        assert_eq!(
            issue.with_value_as_after(&diff),
            description_diff("before", "server")
        );
    }
}
