use super::{
    Action, Dispatcher, IssueAction, JournalAction, JournalEntry, LocalJournalState,
    RemoteJournalState, Store, journal_store::merge_fetched_journals,
};
use crate::entities::LocalJournal;
use crate::libs::yaml::parse_journal_yaml;
use crate::test_support::sample_issue_aggregate;
use crate::vos::{
    IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId, JournalKey, LocalJournalId,
};

fn remote_edited_dispatcher() -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: "edited notes".to_string(),
    });
    dispatcher.consume_action();
    dispatcher
}

fn local_only_dispatcher() -> (Dispatcher, LocalJournalId) {
    let mut dispatcher = Dispatcher::new();
    let id = dispatcher.new_local_journal_id();
    dispatcher.dispatch(JournalAction::CreateLocal {
        id,
        issue_id: IssueId::new(1),
        notes: "local notes".to_string(),
    });
    dispatcher.consume_action();
    (dispatcher, id)
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .expect("panic should contain a string message")
}

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
fn register_remote_adds_a_synced_entry_with_its_owner() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();

    let Some(JournalEntry::Remote {
        journal,
        issue_id,
        state,
        notes_diff,
    }) = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
    else {
        panic!("registered remote journal should exist");
    };
    assert_eq!(journal.id, JournalId::new(1));
    assert_eq!(*issue_id, IssueId::new(3));
    assert_eq!(state, &RemoteJournalState::Synced);
    assert_eq!(notes_diff, &None);
}

#[test]
fn register_remote_rejects_an_id_owned_by_another_issue_without_replacing_it() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(4),
    });

    let result = catch_unwind(AssertUnwindSafe(|| dispatcher.consume_action()));

    assert_eq!(
        panic_message(result.unwrap_err()),
        "remote journal is already owned by another issue"
    );
    let Some(JournalEntry::Remote { issue_id, .. }) = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
    else {
        panic!("original remote journal should remain stored");
    };
    assert_eq!(*issue_id, IssueId::new(3));
}

#[test]
fn register_remote_rejects_existing_synced_edited_and_uploading_entries_without_changes() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    for uploading in [false, true] {
        let mut dispatcher = remote_edited_dispatcher();
        if uploading {
            dispatcher.dispatch(JournalAction::StartUpload {
                key: JournalKey::Remote(JournalId::new(1)),
            });
            dispatcher.consume_action();
        }
        dispatcher.dispatch(JournalAction::RegisterRemote {
            journal: parse_journal_yaml(JournalId::new(1)),
            issue_id: IssueId::new(3),
        });

        assert!(catch_unwind(AssertUnwindSafe(|| dispatcher.consume_action())).is_err());
        let Some(JournalEntry::Remote {
            journal,
            state,
            notes_diff,
            ..
        }) = dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        else {
            panic!("existing remote journal should remain stored");
        };
        assert_eq!(journal.notes, "edited notes");
        assert_eq!(notes_diff.as_ref().unwrap().after, "edited notes");
        assert_eq!(
            state,
            if uploading {
                &RemoteJournalState::Uploading
            } else {
                &RemoteJournalState::Edited
            }
        );
    }

    let mut synced = Dispatcher::new();
    synced.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });
    synced.consume_action();
    synced.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });
    assert!(catch_unwind(AssertUnwindSafe(|| synced.consume_action())).is_err());
    assert!(matches!(
        synced
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
        Some(JournalEntry::Remote {
            state: RemoteJournalState::Synced,
            notes_diff: None,
            ..
        })
    ));
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
fn edit_remote_notes_creates_a_diff_and_updates_visible_notes() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
    let before = parse_journal_yaml(JournalId::new(1)).notes;

    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: "edited notes".to_string(),
    });
    dispatcher.consume_action();

    let JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    } = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("edited remote journal should remain stored")
    else {
        panic!("remote key should refer to a remote journal");
    };
    assert_eq!(journal.notes, "edited notes");
    assert_eq!(state, &RemoteJournalState::Edited);
    let diff = notes_diff.as_ref().expect("an edit should create a diff");
    assert_eq!(diff.before, before);
    assert_eq!(diff.after, "edited notes");
}

#[test]
fn editing_synced_remote_notes_to_the_same_value_keeps_it_synced_without_a_diff() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
    let notes = parse_journal_yaml(JournalId::new(1)).notes;

    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: notes.clone(),
    });
    dispatcher.consume_action();

    let JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    } = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("unchanged remote journal should remain stored")
    else {
        panic!("remote key should refer to a remote journal");
    };
    assert_eq!(journal.notes, notes);
    assert_eq!(state, &RemoteJournalState::Synced);
    assert_eq!(notes_diff, &None);
}

#[test]
fn repeated_remote_edits_keep_the_first_before_and_latest_after() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
    let before = parse_journal_yaml(JournalId::new(1)).notes;
    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: "first edit".to_string(),
    });
    dispatcher.consume_action();

    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: "latest edit".to_string(),
    });
    dispatcher.consume_action();

    let JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    } = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("edited remote journal should remain stored")
    else {
        panic!("remote key should refer to a remote journal");
    };
    assert_eq!(journal.notes, "latest edit");
    assert_eq!(state, &RemoteJournalState::Edited);
    let diff = notes_diff.as_ref().expect("edits should keep a diff");
    assert_eq!(diff.before, before);
    assert_eq!(diff.after, "latest edit");
}

#[test]
fn reverting_remote_notes_clears_the_diff_and_restores_synced_state() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    dispatcher.consume_action();
    let before = parse_journal_yaml(JournalId::new(1)).notes;
    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: "edited notes".to_string(),
    });
    dispatcher.consume_action();

    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(1),
        notes: before.clone(),
    });
    dispatcher.consume_action();

    let JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    } = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        .expect("reverted remote journal should remain stored")
    else {
        panic!("remote key should refer to a remote journal");
    };
    assert_eq!(journal.notes, before);
    assert_eq!(state, &RemoteJournalState::Synced);
    assert_eq!(notes_diff, &None);
}

#[test]
#[should_panic(expected = "remote journal does not exist")]
fn edit_remote_notes_rejects_a_missing_id() {
    let mut dispatcher = Dispatcher::new();

    dispatcher.dispatch(JournalAction::EditRemoteNotes {
        id: JournalId::new(999),
        notes: "edited notes".to_string(),
    });
    dispatcher.consume_action();
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
    other_issue
        .journal_keys
        .push(JournalKey::Remote(JournalId::new(1)));
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

#[test]
fn start_upload_transitions_edited_remote_and_local_only_entries() {
    let mut remote = remote_edited_dispatcher();
    remote.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    remote.consume_action();
    let (mut local, id) = local_only_dispatcher();
    local.dispatch(JournalAction::StartUpload {
        key: JournalKey::Local(id),
    });
    local.consume_action();

    assert!(matches!(
        remote
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
        Some(JournalEntry::Remote {
            state: RemoteJournalState::Uploading,
            ..
        })
    ));
    assert!(matches!(
        local.store().get_journal_entry(JournalKey::Local(id)),
        Some(JournalEntry::Local {
            state: LocalJournalState::Uploading,
            ..
        })
    ));
}

#[test]
fn start_upload_is_a_no_op_for_synced_remote_and_uploading_entries() {
    let mut synced = Dispatcher::new();
    synced.dispatch(IssueAction::Load {
        id: IssueId::new(3),
    });
    synced.consume_action();
    synced.dispatch(Action::LoadJournal {
        id: JournalId::new(1),
    });
    synced.consume_action();
    synced.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    synced.consume_action();
    assert!(matches!(
        synced
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
        Some(JournalEntry::Remote {
            state: RemoteJournalState::Synced,
            ..
        })
    ));

    let mut uploading = remote_edited_dispatcher();
    let action = JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    };
    uploading.dispatch(action);
    uploading.consume_action();
    uploading.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    uploading.consume_action();
    assert!(matches!(
        uploading
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
        Some(JournalEntry::Remote {
            state: RemoteJournalState::Uploading,
            ..
        })
    ));

    let (mut local, id) = local_only_dispatcher();
    local.dispatch(JournalAction::StartUpload {
        key: JournalKey::Local(id),
    });
    local.consume_action();
    local.dispatch(JournalAction::StartUpload {
        key: JournalKey::Local(id),
    });
    local.consume_action();
    assert!(matches!(
        local.store().get_journal_entry(JournalKey::Local(id)),
        Some(JournalEntry::Local {
            state: LocalJournalState::Uploading,
            ..
        })
    ));
}

#[test]
#[should_panic(expected = "journal does not exist")]
fn start_upload_rejects_a_missing_key() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(999)),
    });
    dispatcher.consume_action();
}

#[test]
fn failed_upload_restores_the_editable_state_and_content() {
    let mut remote = remote_edited_dispatcher();
    remote.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    remote.consume_action();
    remote.dispatch(JournalAction::FailUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    remote.consume_action();
    let Some(JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    }) = remote
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
    else {
        panic!()
    };
    assert_eq!(journal.notes, "edited notes");
    assert_eq!(state, &RemoteJournalState::Edited);
    assert_eq!(notes_diff.as_ref().unwrap().after, "edited notes");

    let (mut local, id) = local_only_dispatcher();
    local.dispatch(JournalAction::StartUpload {
        key: JournalKey::Local(id),
    });
    local.consume_action();
    local.dispatch(JournalAction::FailUpload {
        key: JournalKey::Local(id),
    });
    local.consume_action();
    let Some(JournalEntry::Local { journal, state }) =
        local.store().get_journal_entry(JournalKey::Local(id))
    else {
        panic!()
    };
    assert_eq!(journal.notes, "local notes");
    assert_eq!(state, &LocalJournalState::LocalOnly);
}

#[test]
fn fail_upload_rejects_a_non_uploading_entry_without_changing_its_state() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let (mut dispatcher, id) = local_only_dispatcher();
    let result = catch_unwind(AssertUnwindSafe(|| {
        dispatcher.dispatch(JournalAction::FailUpload {
            key: JournalKey::Local(id),
        });
        dispatcher.consume_action();
    }));
    assert_eq!(
        panic_message(result.unwrap_err()),
        "journal is not uploading"
    );
    assert!(matches!(
        dispatcher.store().get_journal_entry(JournalKey::Local(id)),
        Some(JournalEntry::Local {
            state: LocalJournalState::LocalOnly,
            ..
        })
    ));
}

#[test]
#[should_panic(expected = "journal is not uploading")]
fn fail_upload_rejects_an_edited_remote_entry() {
    let mut dispatcher = remote_edited_dispatcher();
    dispatcher.dispatch(JournalAction::FailUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    dispatcher.consume_action();
}

#[test]
#[should_panic(expected = "journal does not exist")]
fn fail_upload_rejects_a_missing_key() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::FailUpload {
        key: JournalKey::Local(LocalJournalId::new(999)),
    });
    dispatcher.consume_action();
}

#[test]
fn uploading_entries_reject_edits_without_changing_content() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut remote = remote_edited_dispatcher();
    remote.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    remote.consume_action();
    let result = catch_unwind(AssertUnwindSafe(|| {
        remote.dispatch(JournalAction::EditRemoteNotes {
            id: JournalId::new(1),
            notes: "later".to_string(),
        });
        remote.consume_action();
    }));
    assert_eq!(
        panic_message(result.unwrap_err()),
        "cannot edit a remote journal while uploading"
    );
    let Some(JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    }) = remote
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
    else {
        panic!()
    };
    assert_eq!(journal.notes, "edited notes");
    assert_eq!(state, &RemoteJournalState::Uploading);
    assert_eq!(notes_diff.as_ref().unwrap().after, "edited notes");

    let (mut local, id) = local_only_dispatcher();
    local.dispatch(JournalAction::StartUpload {
        key: JournalKey::Local(id),
    });
    local.consume_action();
    let result = catch_unwind(AssertUnwindSafe(|| {
        local.dispatch(JournalAction::EditLocalNotes {
            id,
            notes: "later".to_string(),
        });
        local.consume_action();
    }));
    assert_eq!(
        panic_message(result.unwrap_err()),
        "cannot edit a local journal while uploading"
    );
    let Some(JournalEntry::Local { journal, state }) =
        local.store().get_journal_entry(JournalKey::Local(id))
    else {
        panic!()
    };
    assert_eq!(journal.notes, "local notes");
    assert_eq!(state, &LocalJournalState::Uploading);
}

#[test]
fn complete_remote_upload_preserves_metadata_and_clears_the_diff() {
    let expected = parse_journal_yaml(JournalId::new(1));
    let mut dispatcher = remote_edited_dispatcher();
    dispatcher.dispatch(JournalAction::StartUpload {
        key: JournalKey::Remote(JournalId::new(1)),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::CompleteRemoteUpload {
        id: JournalId::new(1),
        notes: "sent notes".to_string(),
    });
    dispatcher.consume_action();

    let Some(JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    }) = dispatcher
        .store()
        .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
    else {
        panic!()
    };
    assert_eq!(journal.id, expected.id);
    assert_eq!(journal.user, expected.user);
    assert_eq!(journal.updated_on, expected.updated_on);
    let [JournalDetail::Attr(JournalDetailAttr::StatusId { old, new })] =
        journal.details.as_slice()
    else {
        panic!("completed upload should preserve journal details");
    };
    assert_eq!(*old, IssueStatusId::new(1));
    assert_eq!(*new, IssueStatusId::new(2));
    assert_eq!(journal.notes, "sent notes");
    assert_eq!(state, &RemoteJournalState::Synced);
    assert_eq!(notes_diff, &None);
}

#[test]
#[should_panic(expected = "remote journal is not uploading")]
fn complete_remote_upload_rejects_a_non_uploading_entry() {
    let mut dispatcher = remote_edited_dispatcher();
    dispatcher.dispatch(JournalAction::CompleteRemoteUpload {
        id: JournalId::new(1),
        notes: "sent".to_string(),
    });
    dispatcher.consume_action();
}

#[test]
#[should_panic(expected = "remote journal does not exist")]
fn complete_remote_upload_rejects_a_missing_id() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::CompleteRemoteUpload {
        id: JournalId::new(999),
        notes: "sent".to_string(),
    });
    dispatcher.consume_action();
}

#[test]
fn fetched_merge_uses_server_order_and_preserves_dirty_and_local_entries() {
    use std::collections::HashMap;

    let issue_id = IssueId::new(3);
    let local_id = LocalJournalId::new(1);
    let mut edited = parse_journal_yaml(JournalId::new(1));
    edited.notes = "local edit".to_string();
    let uploading = parse_journal_yaml(JournalId::new(2));
    let mut edited_four = parse_journal_yaml(JournalId::new(3));
    edited_four.id = JournalId::new(4);
    edited_four.notes = "second local edit".to_string();
    let entries = HashMap::from([
        (
            JournalKey::Remote(JournalId::new(4)),
            JournalEntry::Remote {
                journal: edited_four,
                issue_id,
                state: RemoteJournalState::Edited,
                notes_diff: Some(crate::vos::JournalNotesDiff {
                    before: "second server before".to_string(),
                    after: "second local edit".to_string(),
                }),
            },
        ),
        (
            JournalKey::Remote(JournalId::new(1)),
            JournalEntry::Remote {
                journal: edited,
                issue_id,
                state: RemoteJournalState::Edited,
                notes_diff: Some(crate::vos::JournalNotesDiff {
                    before: "server before".to_string(),
                    after: "local edit".to_string(),
                }),
            },
        ),
        (
            JournalKey::Remote(JournalId::new(2)),
            JournalEntry::Remote {
                journal: uploading,
                issue_id,
                state: RemoteJournalState::Uploading,
                notes_diff: Some(crate::vos::JournalNotesDiff {
                    before: "before upload".to_string(),
                    after: "uploading edit".to_string(),
                }),
            },
        ),
        (
            JournalKey::Local(local_id),
            JournalEntry::Local {
                journal: LocalJournal {
                    id: local_id,
                    issue_id,
                    notes: "new notes".to_string(),
                },
                state: LocalJournalState::LocalOnly,
            },
        ),
    ]);
    let mut server_two = parse_journal_yaml(JournalId::new(2));
    server_two.notes = "server changed".to_string();
    let result = merge_fetched_journals(
        issue_id,
        vec![server_two, parse_journal_yaml(JournalId::new(3))],
        &[
            JournalKey::Remote(JournalId::new(1)),
            JournalKey::Remote(JournalId::new(4)),
            JournalKey::Remote(JournalId::new(2)),
            JournalKey::Local(local_id),
        ],
        &entries,
    );

    assert_eq!(
        result.journal_keys,
        vec![
            JournalKey::Remote(JournalId::new(2)),
            JournalKey::Remote(JournalId::new(3)),
            JournalKey::Remote(JournalId::new(1)),
            JournalKey::Remote(JournalId::new(4)),
            JournalKey::Local(local_id),
        ]
    );
    let JournalEntry::Remote { journal, state, .. } = result
        .entries
        .get(&JournalKey::Remote(JournalId::new(2)))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(journal.notes, parse_journal_yaml(JournalId::new(2)).notes);
    assert_eq!(state, &RemoteJournalState::Uploading);
    let JournalEntry::Remote { journal, state, .. } = result
        .entries
        .get(&JournalKey::Remote(JournalId::new(1)))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(journal.notes, "local edit");
    assert_eq!(state, &RemoteJournalState::Edited);
}

#[test]
fn fetched_merge_replaces_synced_entries_and_removes_missing_synced_entries() {
    use std::collections::HashMap;

    let issue_id = IssueId::new(3);
    let entries = HashMap::from([
        (
            JournalKey::Remote(JournalId::new(1)),
            JournalEntry::Remote {
                journal: parse_journal_yaml(JournalId::new(1)),
                issue_id,
                state: RemoteJournalState::Synced,
                notes_diff: None,
            },
        ),
        (
            JournalKey::Remote(JournalId::new(2)),
            JournalEntry::Remote {
                journal: parse_journal_yaml(JournalId::new(2)),
                issue_id,
                state: RemoteJournalState::Synced,
                notes_diff: None,
            },
        ),
    ]);
    let mut fetched = parse_journal_yaml(JournalId::new(1));
    fetched.notes = "fresh server notes".to_string();
    let result = merge_fetched_journals(
        issue_id,
        vec![fetched],
        &[
            JournalKey::Remote(JournalId::new(1)),
            JournalKey::Remote(JournalId::new(2)),
        ],
        &entries,
    );

    assert_eq!(
        result.journal_keys,
        vec![JournalKey::Remote(JournalId::new(1))]
    );
    let JournalEntry::Remote {
        journal,
        state,
        notes_diff,
        ..
    } = result
        .entries
        .get(&JournalKey::Remote(JournalId::new(1)))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(journal.notes, "fresh server notes");
    assert_eq!(state, &RemoteJournalState::Synced);
    assert_eq!(notes_diff, &None);
    assert!(
        !result
            .entries
            .contains_key(&JournalKey::Remote(JournalId::new(2)))
    );
}

#[test]
fn fetched_merge_rejects_an_id_owned_by_another_issue_without_changing_entries() {
    use std::collections::HashMap;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let key = JournalKey::Remote(JournalId::new(1));
    let entries = HashMap::from([(
        key,
        JournalEntry::Remote {
            journal: parse_journal_yaml(JournalId::new(1)),
            issue_id: IssueId::new(4),
            state: RemoteJournalState::Synced,
            notes_diff: None,
        },
    )]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        merge_fetched_journals(
            IssueId::new(3),
            vec![parse_journal_yaml(JournalId::new(1))],
            &[],
            &entries,
        )
    }));

    let payload = match result {
        Err(payload) => payload,
        Ok(_) => panic!("ownership conflict should be rejected"),
    };
    assert_eq!(
        panic_message(payload),
        "remote journal is already owned by another issue"
    );
    let JournalEntry::Remote { issue_id, .. } = entries.get(&key).unwrap() else {
        panic!()
    };
    assert_eq!(*issue_id, IssueId::new(4));
}

#[test]
fn fetched_merge_rejects_duplicate_fetched_ids_before_building_a_result() {
    use std::collections::HashMap;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let entries = HashMap::new();
    let result = catch_unwind(AssertUnwindSafe(|| {
        merge_fetched_journals(
            IssueId::new(3),
            vec![
                parse_journal_yaml(JournalId::new(1)),
                parse_journal_yaml(JournalId::new(1)),
            ],
            &[],
            &entries,
        )
    }));

    let payload = match result {
        Err(payload) => payload,
        Ok(_) => panic!("duplicate fetched IDs should be rejected"),
    };
    assert_eq!(
        panic_message(payload),
        "fetched journals contain duplicate IDs"
    );
    assert!(entries.is_empty());
}

#[test]
fn fetched_merge_rejects_duplicate_old_keys_and_multiple_local_keys() {
    use std::collections::HashMap;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let entries = HashMap::new();
    for (keys, message) in [
        (
            vec![
                JournalKey::Remote(JournalId::new(1)),
                JournalKey::Remote(JournalId::new(1)),
            ],
            "old journal keys contain duplicates",
        ),
        (
            vec![
                JournalKey::Local(LocalJournalId::new(1)),
                JournalKey::Local(LocalJournalId::new(2)),
            ],
            "old journal keys contain multiple local journals",
        ),
    ] {
        let result = catch_unwind(AssertUnwindSafe(|| {
            merge_fetched_journals(IssueId::new(3), vec![], &keys, &entries)
        }));
        let payload = match result {
            Err(payload) => payload,
            Ok(_) => panic!("invalid old keys should be rejected"),
        };
        assert_eq!(panic_message(payload), message);
    }
}

#[test]
fn sync_fetched_remote_registers_missing_and_replaces_synced_entries() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();
    let mut replacement = parse_journal_yaml(JournalId::new(1));
    replacement.notes = "fresh".to_string();
    dispatcher.dispatch(JournalAction::SyncFetchedRemote {
        journal: replacement,
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::SyncFetchedRemote {
        journal: parse_journal_yaml(JournalId::new(2)),
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();

    for (id, notes) in [
        (1, "fresh"),
        (2, parse_journal_yaml(JournalId::new(2)).notes.as_str()),
    ] {
        let Some(JournalEntry::Remote {
            journal,
            state,
            notes_diff,
            ..
        }) = dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(id)))
        else {
            panic!()
        };
        assert_eq!(journal.notes, notes);
        assert_eq!(state, &RemoteJournalState::Synced);
        assert_eq!(notes_diff, &None);
    }
}

#[test]
fn sync_fetched_remote_preserves_edited_and_uploading_entries() {
    for uploading in [false, true] {
        let mut dispatcher = remote_edited_dispatcher();
        if uploading {
            dispatcher.dispatch(JournalAction::StartUpload {
                key: JournalKey::Remote(JournalId::new(1)),
            });
            dispatcher.consume_action();
        }
        dispatcher.dispatch(JournalAction::SyncFetchedRemote {
            journal: parse_journal_yaml(JournalId::new(1)),
            issue_id: IssueId::new(3),
        });
        dispatcher.consume_action();

        let Some(JournalEntry::Remote { journal, state, .. }) = dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        else {
            panic!()
        };
        assert_eq!(journal.notes, "edited notes");
        assert_eq!(
            state,
            if uploading {
                &RemoteJournalState::Uploading
            } else {
                &RemoteJournalState::Edited
            }
        );
    }
}

#[test]
fn sync_fetched_remote_rejects_another_owner_without_replacing_it() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(4),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::SyncFetchedRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });

    assert!(catch_unwind(AssertUnwindSafe(|| dispatcher.consume_action())).is_err());
    assert!(matches!(
        dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
        Some(JournalEntry::Remote { issue_id, .. }) if *issue_id == IssueId::new(4)
    ));
}

#[test]
fn remove_synced_remote_removes_only_an_owned_synced_entry() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();
    dispatcher.dispatch(JournalAction::RemoveSyncedRemote {
        id: JournalId::new(1),
        issue_id: IssueId::new(3),
    });
    dispatcher.consume_action();

    assert!(
        dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
            .is_none()
    );
}

#[test]
fn remove_synced_remote_rejects_dirty_other_owner_missing_and_local_entries() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    for uploading in [false, true] {
        let mut dirty = remote_edited_dispatcher();
        if uploading {
            dirty.dispatch(JournalAction::StartUpload {
                key: JournalKey::Remote(JournalId::new(1)),
            });
            dirty.consume_action();
        }
        dirty.dispatch(JournalAction::RemoveSyncedRemote {
            id: JournalId::new(1),
            issue_id: IssueId::new(3),
        });
        let result = catch_unwind(AssertUnwindSafe(|| dirty.consume_action()));
        assert_eq!(
            panic_message(result.unwrap_err()),
            "cannot remove a dirty remote journal"
        );
        assert!(matches!(
            dirty
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
            Some(JournalEntry::Remote {
                journal,
                state,
                notes_diff: Some(diff),
                ..
            }) if journal.notes == "edited notes"
                && diff.after == "edited notes"
                && state == if uploading {
                    &RemoteJournalState::Uploading
                } else {
                    &RemoteJournalState::Edited
                }
        ));
    }

    let mut other = Dispatcher::new();
    other.dispatch(JournalAction::RegisterRemote {
        journal: parse_journal_yaml(JournalId::new(1)),
        issue_id: IssueId::new(4),
    });
    other.consume_action();
    other.dispatch(JournalAction::RemoveSyncedRemote {
        id: JournalId::new(1),
        issue_id: IssueId::new(3),
    });
    let result = catch_unwind(AssertUnwindSafe(|| other.consume_action()));
    assert_eq!(
        panic_message(result.unwrap_err()),
        "remote journal belongs to another issue"
    );
    assert!(matches!(
        other
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
        Some(JournalEntry::Remote {
            issue_id,
            state: RemoteJournalState::Synced,
            notes_diff: None,
            ..
        }) if *issue_id == IssueId::new(4)
    ));

    let (mut local, local_id) = local_only_dispatcher();
    local.dispatch(JournalAction::RemoveSyncedRemote {
        id: JournalId::new(1),
        issue_id: IssueId::new(1),
    });
    let result = catch_unwind(AssertUnwindSafe(|| local.consume_action()));
    assert_eq!(
        panic_message(result.unwrap_err()),
        "remote journal does not exist"
    );
    assert!(
        local
            .store()
            .get_journal_entry(JournalKey::Local(local_id))
            .is_some()
    );

    let mut missing = Dispatcher::new();
    missing.dispatch(JournalAction::RemoveSyncedRemote {
        id: JournalId::new(99),
        issue_id: IssueId::new(3),
    });
    let result = catch_unwind(AssertUnwindSafe(|| missing.consume_action()));
    assert_eq!(
        panic_message(result.unwrap_err()),
        "remote journal does not exist"
    );
    assert!(
        missing
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(99)))
            .is_none()
    );
}
