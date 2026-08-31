pub mod category;
pub mod issue;
pub mod issue_aggregate;
pub mod issue_status;
pub mod journal;
pub mod priority;
pub mod project;
pub mod target_version;
pub mod time_entity_activity;
pub mod tracker;
pub mod user;

pub use category::Category;
pub use issue::{Issue, ProjectIssuesPage};
pub use issue_aggregate::IssueAggregate;
pub use issue_status::IssueStatus;
pub use journal::Journal;
pub use priority::Priority;
pub use project::Project;
pub use target_version::TargetVersion;
pub use time_entity_activity::TimeEntityActivity;
pub use tracker::Tracker;
pub use user::User;

#[cfg(test)]
mod issue_aggregate_export_tests {
    use super::IssueAggregate;

    #[test]
    fn exports_issue_aggregate() {
        fn accepts_issue_aggregate(_: IssueAggregate) {}
        let _ = accepts_issue_aggregate;
    }
}

#[cfg(test)]
mod issue_export_tests {
    use super::Issue;
    use crate::vos::{IssueId, IssueStatusId, ProjectId};

    #[test]
    fn exports_lightweight_issue_with_id() {
        let issue = Issue {
            id: IssueId::new(1),
            project_id: ProjectId::new(2),
            subject: "subject".to_string(),
            description: "description".to_string(),
            status_id: IssueStatusId::new(3),
        };

        assert_eq!(issue.id, IssueId::new(1));
    }
}
