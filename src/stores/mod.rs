mod issue_store;
mod project_issues_store;
mod store;

pub use issue_store::{IssueAction, IssueState};
pub use project_issues_store::{
    ProjectIssuesAction, ProjectIssuesPageState, ProjectIssuesRequestId,
};
pub use store::{Action, Dispatcher, JournalState, Store};

#[cfg(test)]
mod issue_store_tests;
#[cfg(test)]
mod project_issues_store_tests;

#[cfg(test)]
mod tests {
    use super::{
        Action, Dispatcher, IssueAction, IssueState, JournalState, ProjectIssuesRequestId, Store,
    };

    #[test]
    fn store_types_are_available_from_the_stores_module() {
        let _ = Dispatcher::new();
        let _ = Store::new();
        let _: Option<Action> = None;
        let _: Option<IssueAction> = None;
        let _: Option<IssueState> = None;
        let _: Option<JournalState> = None;
        let _: Option<ProjectIssuesRequestId> = None;
    }
}
