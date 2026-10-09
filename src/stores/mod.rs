mod issue_journals;
mod issue_store;
mod journal_state;
mod notice_store;
mod project_issues_store;
mod store;

pub use issue_journals::JournalAction;
pub use issue_store::{IssueAction, IssueFetchState, IssueState};
pub use journal_state::{
    DeletedJournalEntry, DeletedJournalState, JournalUploadFailure, LocalJournalEntry,
    LocalJournalState, RemoteJournalState, RemoteJournalUploadConflict, RemoteJournalView,
};
pub use project_issues_store::{
    ProjectIssuesAction, ProjectIssuesPageState, ProjectIssuesRequestId,
};
pub use store::{Action, Dispatcher, Store};
// Journal保存などの失敗発生源が接続されるまで、Notice系の公開APIは主にテストから利用される。
pub use notice_store::{Notice, NoticeAction, NoticeId};

#[cfg(test)]
mod issue_journals_tests;
#[cfg(test)]
mod issue_store_tests;
#[cfg(test)]
mod notice_store_tests;
#[cfg(test)]
mod project_issues_store_tests;

#[cfg(test)]
mod tests {
    use super::{
        Action, Dispatcher, IssueAction, IssueState, JournalAction, NoticeId,
        ProjectIssuesRequestId, Store,
    };

    #[test]
    fn store_types_are_available_from_the_stores_module() {
        let _ = Dispatcher::new();
        let _ = Store::new();
        let _: Option<Action> = None;
        let _: Option<IssueAction> = None;
        let _: Option<IssueState> = None;
        let _: Option<JournalAction> = None;
        let _: Option<ProjectIssuesRequestId> = None;
        let _: Option<NoticeId> = None;
    }
}
