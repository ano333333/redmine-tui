mod issue_store;
mod store;

pub use issue_store::{IssueAction, IssueState};
pub use store::{Action, Dispatcher, JournalState, Store};

#[cfg(test)]
mod issue_store_tests;

#[cfg(test)]
mod tests {
    use super::{Action, Dispatcher, IssueAction, IssueState, JournalState, Store};

    #[test]
    fn store_types_are_available_from_the_stores_module() {
        let _ = Dispatcher::new();
        let _ = Store::new();
        let _: Option<Action> = None;
        let _: Option<IssueAction> = None;
        let _: Option<IssueState> = None;
        let _: Option<JournalState> = None;
    }
}
