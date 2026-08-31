use super::{
    Action, Dispatcher, IssueAction, JournalAction, JournalEntry, LocalJournalState,
    RemoteJournalState, Store,
};
use crate::libs::yaml::parse_journal_yaml;
use crate::test_support::sample_issue_aggregate;
use crate::vos::{
    IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId, JournalKey, LocalJournalId,
};

#[test]
fn empty_store_has_no_remote_or_local_entry() {
    let store = Store::new();

    assert!(
        store
            .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
            .is_none()
    );
    assert!(
        store
            .get_journal_entry(JournalKey::Local(LocalJournalId::new(1)))
            .is_none()
    );
}

#[test]
fn load_journal_adds_a_remote_entry_for_its_loaded_issue() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();

    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();

    let entry = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("fixture journal should be stored as a remote entry");
    let JournalEntry::Remote {
        journal,
        issue_id,
        state,
        notes_diff,
    } = entry
    else {
        panic!("remote key should refer to a remote journal");
    };
    let expected = parse_journal_yaml(JournalId::new(1));
    assert_eq!(journal.id, expected.id);
    assert_eq!(journal.user, expected.user);
    assert_eq!(journal.updated_on, expected.updated_on);
    let [JournalDetail::Attr(JournalDetailAttr::StatusId { old, new })] =
        journal.details.as_slice()
    else {
        panic!("fixture journal should retain its status detail");
    };
    assert_eq!(*old, IssueStatusId::new(1));
    assert_eq!(*new, IssueStatusId::new(2));
    assert_eq!(journal.notes, expected.notes);
    assert_eq!(*issue_id, IssueId::new(3));
    assert_eq!(state, &RemoteJournalState::Synced);
    assert_eq!(notes_diff, &None);
}

#[test]
#[should_panic(expected = "fixture journal must belong to exactly one loaded issue")]
fn load_journal_rejects_a_missing_owner() {
    let mut dispatcher = Dispatcher::new();

    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
}

#[test]
#[should_panic(expected = "fixture journal must belong to exactly one loaded issue")]
fn load_journal_rejects_multiple_owners() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    let mut other_issue =
        sample_issue_aggregate(2, "other", IssueStatusId::new(1), None, None, None, 0);
    other_issue.journal_ids.push(JournalId::new(1));
    dispatcher.dispatch(IssueAction::Sync { issue: other_issue });
    dispatcher.consume_action();

    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
}

#[test]
fn loading_the_same_remote_journal_keeps_the_existing_entry() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
    let original_user_address = match dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("fixture journal should be stored")
    {
        JournalEntry::Remote { journal, .. } => journal.user.as_ptr(),
        JournalEntry::Local { .. } => panic!("remote key should refer to a remote journal"),
    };

    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();

    let reloaded_user_address = match dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("fixture journal should remain stored")
    {
        JournalEntry::Remote { journal, .. } => journal.user.as_ptr(),
        JournalEntry::Local { .. } => panic!("remote key should refer to a remote journal"),
    };
    assert_eq!(reloaded_user_address, original_user_address);
}

#[test]
fn dispatcher_issues_monotonic_ids_without_reusing_an_unused_id() {
    let mut dispatcher = Dispatcher::new();

    let unused = dispatcher.new_local_journal_id();
    let next = dispatcher.new_local_journal_id();

    assert_eq!(unused, LocalJournalId::new(1));
    assert_eq!(next, LocalJournalId::new(2));
}

#[test]
fn create_local_adds_an_entry_through_dispatch_and_consume() {
    let mut dispatcher = Dispatcher::new();
    let id = dispatcher.new_local_journal_id();
    let issue_id = IssueId::new(1);

    dispatcher.dispatch(JournalAction::CreateLocal {
        id,
        issue_id,
        notes: "new notes".to_string(),
    });
    dispatcher.consume_action();

    let entry = dispatcher
        .store()
        .get_journal_entry(JournalKey::Local(id))
        .expect("created local journal should be stored");
    let JournalEntry::Local { journal, state } = entry else {
        panic!("local key should refer to a local journal");
    };
    assert_eq!(journal.id, id);
    assert_eq!(journal.issue_id, issue_id);
    assert_eq!(journal.notes, "new notes");
    assert_eq!(state, &LocalJournalState::LocalOnly);
}

#[test]
fn edit_local_notes_replaces_notes_and_keeps_local_only_state() {
    let mut dispatcher = Dispatcher::new();
    let id = dispatcher.new_local_journal_id();

    dispatcher.dispatch(JournalAction::CreateLocal {
        id,
        issue_id: IssueId::new(1),
        notes: "before".to_string(),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::EditLocalNotes {
        id,
        notes: "after".to_string(),
    });
    dispatcher.consume_action();

    let entry = dispatcher
        .store()
        .get_journal_entry(JournalKey::Local(id))
        .expect("edited local journal should remain stored");
    let JournalEntry::Local { journal, state } = entry else {
        panic!("local key should refer to a local journal");
    };
    assert_eq!(journal.notes, "after");
    assert_eq!(state, &LocalJournalState::LocalOnly);
}

#[test]
#[should_panic(expected = "local journal does not exist")]
fn edit_local_notes_rejects_a_missing_id() {
    let mut dispatcher = Dispatcher::new();
    let id = dispatcher.new_local_journal_id();

    dispatcher.dispatch(JournalAction::EditLocalNotes {
        id,
        notes: "after".to_string(),
    });
    dispatcher.consume_action();
}

#[test]
#[should_panic(expected = "issue already has a local journal")]
fn create_local_rejects_a_second_local_journal_for_the_same_issue() {
    let mut dispatcher = Dispatcher::new();
    let first = dispatcher.new_local_journal_id();
    let second = dispatcher.new_local_journal_id();

    dispatcher.dispatch(JournalAction::CreateLocal {
        id: first,
        issue_id: IssueId::new(1),
        notes: String::new(),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::CreateLocal {
        id: second,
        issue_id: IssueId::new(1),
        notes: String::new(),
    });
    dispatcher.consume_action();
}

#[test]
#[should_panic(expected = "local journal ID was not issued by this Store")]
fn create_local_rejects_an_unissued_id() {
    let mut dispatcher = Dispatcher::new();

    dispatcher.dispatch(JournalAction::CreateLocal {
        id: LocalJournalId::new(999),
        issue_id: IssueId::new(1),
        notes: String::new(),
    });
    dispatcher.consume_action();
}

#[test]
#[should_panic(expected = "local journal ID has already been used")]
fn create_local_rejects_reusing_an_issued_id() {
    let mut dispatcher = Dispatcher::new();
    let id = dispatcher.new_local_journal_id();

    dispatcher.dispatch(JournalAction::CreateLocal {
        id,
        issue_id: IssueId::new(1),
        notes: String::new(),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::CreateLocal {
        id,
        issue_id: IssueId::new(2),
        notes: String::new(),
    });
    dispatcher.consume_action();
}
