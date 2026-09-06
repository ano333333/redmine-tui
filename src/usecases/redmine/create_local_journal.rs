use crate::stores::{Action, Dispatcher, IssueAction, JournalAction};
use crate::vos::{IssueId, JournalKey};

/// Creates a Local Journal for an Issue that has no Local Journal yet.
///
/// Returns the created `JournalKey::Local` and the ordered actions for the
/// caller to dispatch. This function does not dispatch anything itself.
/// When the Issue already has a Local Journal, `None` is returned.
pub fn create_local_journal(
    dispatcher: &mut Dispatcher,
    issue_id: IssueId,
) -> Option<(JournalKey, Vec<Action>)> {
    let journal_keys = dispatcher
        .store()
        .get_issue(issue_id)
        .expect("cannot create a local journal for a missing issue")
        .0
        .journal_keys
        .clone();
    if journal_keys
        .iter()
        .any(|key| matches!(key, JournalKey::Local(_)))
    {
        return None;
    }
    let id = dispatcher.new_local_journal_id();
    let key = JournalKey::Local(id);
    let actions = vec![
        JournalAction::CreateLocal {
            id,
            issue_id,
            notes: String::new(),
        }
        .into(),
        IssueAction::AppendJournalKey { issue_id, key }.into(),
    ];
    Some((key, actions))
}

#[cfg(test)]
mod tests {
    use crate::stores::{Dispatcher, IssueAction};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId, JournalKey, LocalJournalId};

    use super::{Action, JournalAction, create_local_journal};

    fn dispatcher_with_issue(journal_keys: Vec<JournalKey>) -> Dispatcher {
        let mut issue =
            sample_issue_aggregate(3, "issue", IssueStatusId::new(1), None, None, None, 0);
        issue.journal_keys = journal_keys;
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(IssueAction::Sync { issue });
        dispatcher.consume_action();
        dispatcher
    }

    #[test]
    fn returns_issued_key_and_create_then_append_actions() {
        let mut dispatcher = dispatcher_with_issue(vec![JournalKey::Remote(JournalId::new(1))]);

        let Some((key, actions)) = create_local_journal(&mut dispatcher, IssueId::new(3)) else {
            panic!("a local journal should be created when none exists")
        };

        assert_eq!(key, JournalKey::Local(LocalJournalId::new(1)));
        let [
            Action::Journal(JournalAction::CreateLocal {
                id,
                issue_id,
                notes,
            }),
            Action::Issue(IssueAction::AppendJournalKey {
                issue_id: append_issue_id,
                key: append_key,
            }),
        ] = actions.as_slice()
        else {
            panic!("expected CreateLocal then AppendJournalKey")
        };
        assert_eq!(*id, LocalJournalId::new(1));
        assert_eq!(*issue_id, IssueId::new(3));
        assert_eq!(notes, "");
        assert_eq!(*append_issue_id, IssueId::new(3));
        assert_eq!(*append_key, JournalKey::Local(LocalJournalId::new(1)));
    }

    #[test]
    fn returns_none_without_starting_when_a_local_journal_already_exists() {
        let mut dispatcher = dispatcher_with_issue(vec![
            JournalKey::Remote(JournalId::new(1)),
            JournalKey::Local(LocalJournalId::new(1)),
        ]);

        assert!(create_local_journal(&mut dispatcher, IssueId::new(3)).is_none());
        assert_eq!(dispatcher.new_local_journal_id(), LocalJournalId::new(1));
    }
}
