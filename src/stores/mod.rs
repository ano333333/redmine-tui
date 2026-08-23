mod store;

pub use store::{Action, Dispatcher, IssueState, JournalState, Store};

#[cfg(test)]
mod tests {
    use super::{Action, Dispatcher, IssueState, JournalState, Store};

    #[test]
    fn store_types_are_available_from_the_stores_module() {
        let _ = Dispatcher::new();
        let _ = Store::new();
        let _: Option<Action> = None;
        let _: Option<IssueState> = None;
        let _: Option<JournalState> = None;
    }
}
