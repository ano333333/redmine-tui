use std::collections::HashMap;

use super::journal_store::{IssueJournals, JournalAction, JournalStore};
use crate::entities::{Journal, LocalJournal};
use crate::stores::journal_state::{
    JournalUploadFailure, LocalJournalEntry, LocalJournalState, RemoteJournalEntry,
    RemoteJournalState,
};
use crate::test_support::local_datetime;
use crate::vos::{IssueId, JournalId, JournalNotesDiff};

fn journal(issue_id: impl Into<IssueId>, journal_id: impl Into<JournalId>) -> Journal {
    let journal_id = journal_id.into();
    journal_with_notes(issue_id, journal_id, &format!("remote notes {journal_id}"))
}

fn journal_with_notes(
    issue_id: impl Into<IssueId>,
    journal_id: impl Into<JournalId>,
    notes: &str,
) -> Journal {
    Journal {
        id: journal_id.into(),
        issue_id: issue_id.into(),
        user: "admin".to_string(),
        updated_on: local_datetime("2026-09-10T00:00:00+09:00"),
        details: vec![],
        notes: notes.to_string(),
    }
}

fn remote_entry(
    issue_id: impl Into<IssueId>,
    journal_id: impl Into<JournalId>,
) -> RemoteJournalEntry {
    RemoteJournalEntry {
        journal: journal(issue_id, journal_id),
        state: RemoteJournalState::Synced,
    }
}

fn edited_remote_entry(
    issue_id: impl Into<IssueId>,
    journal_id: impl Into<JournalId>,
) -> RemoteJournalEntry {
    let journal_id = journal_id.into();
    RemoteJournalEntry {
        journal: journal(issue_id, journal_id),
        state: RemoteJournalState::Edited {
            diff: JournalNotesDiff {
                before: format!("remote notes {journal_id}"),
                after: "edited notes".to_string(),
            },
            failure: None,
        },
    }
}

fn uploading_remote_entry(
    issue_id: impl Into<IssueId>,
    journal_id: impl Into<JournalId>,
) -> RemoteJournalEntry {
    let journal_id = journal_id.into();
    RemoteJournalEntry {
        journal: journal(issue_id, journal_id),
        state: RemoteJournalState::Uploading {
            diff: JournalNotesDiff {
                before: format!("remote notes {journal_id}"),
                after: "uploading notes".to_string(),
            },
            conflict: None,
        },
    }
}

fn local_entry(issue_id: impl Into<IssueId>) -> LocalJournalEntry {
    LocalJournalEntry {
        journal: LocalJournal {
            issue_id: issue_id.into(),
            notes: "local notes".to_string(),
        },
        state: LocalJournalState::Uploading,
    }
}

#[test]
fn getters_on_an_empty_store_return_empty_results() {
    let store = JournalStore::new();
    let issue_id = IssueId::new(1);

    assert!(store.get_remote_journals(issue_id).is_empty());
    assert!(!store.has_remote_journal(issue_id, JournalId::new(10)));
    assert!(store.get_local_journal(issue_id).is_none());
}

#[test]
fn getters_return_journals_stored_via_issue_journals() {
    let issue_id = IssueId::new(1);
    let remote = vec![
        remote_entry(issue_id, JournalId::new(10)),
        remote_entry(issue_id, JournalId::new(11)),
    ];
    let store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote,
                local: Some(local_entry(issue_id)),
            },
        )]),
    };

    let journals = store.get_remote_journals(issue_id);
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(10), JournalId::new(11)]
    );

    let single = store.get_remote_journal(issue_id, 11);
    assert_eq!(single.journal.id, JournalId::new(11));
    assert_eq!(single.journal.issue_id, issue_id);
    assert_eq!(single.journal.notes, "remote notes 11");
    assert!(matches!(single.state, RemoteJournalState::Synced));

    assert!(store.has_remote_journal(issue_id, JournalId::new(11)));

    let fetched_local = store
        .get_local_journal(issue_id)
        .expect("local journal should be stored");
    assert_eq!(fetched_local.journal.issue_id, issue_id);
    assert_eq!(fetched_local.journal.notes, "local notes");
    assert!(matches!(fetched_local.state, LocalJournalState::Uploading));
}

#[test]
fn sync_fetched_registers_journals_on_an_empty_store() {
    let mut store = JournalStore::new();
    let issue_id = IssueId::new(3);

    store.consume_action(JournalAction::SyncFetched {
        issue_id,
        journals: vec![
            journal(issue_id, JournalId::new(10)),
            journal(issue_id, JournalId::new(11)),
        ],
    });

    let journals = store.get_remote_journals(issue_id);
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(10), JournalId::new(11)]
    );
    assert!(
        journals
            .iter()
            .all(|entry| { matches!(entry.state, RemoteJournalState::Synced) })
    );
    assert!(store.get_remote_journals(IssueId::new(4)).is_empty());
    assert!(store.get_local_journal(issue_id).is_none());
}

#[test]
fn sync_fetched_replaces_the_existing_remote_collection_of_the_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(3),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(3), JournalId::new(10))],
                local: Some(local_entry(IssueId::new(3))),
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![journal(IssueId::new(3), JournalId::new(11))],
    });

    let journals = store.get_remote_journals(IssueId::new(3));
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(11)]
    );
    assert!(matches!(
        store
            .get_local_journal(IssueId::new(3))
            .expect("local journal should be kept")
            .state,
        LocalJournalState::Uploading
    ));
}

#[test]
fn sync_fetched_keeps_the_fetched_order_as_the_display_order() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(3),
            IssueJournals {
                remote: vec![
                    remote_entry(IssueId::new(3), JournalId::new(10)),
                    remote_entry(IssueId::new(3), JournalId::new(11)),
                ],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![
            journal(IssueId::new(3), JournalId::new(11)),
            journal(IssueId::new(3), JournalId::new(10)),
        ],
    });

    let journals = store.get_remote_journals(IssueId::new(3));
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(11), JournalId::new(10)]
    );
}

#[test]
fn sync_fetched_replaces_an_existing_synced_journal_with_the_fetched_value() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(3),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(3), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![journal_with_notes(
            IssueId::new(3),
            JournalId::new(10),
            "updated notes",
        )],
    });

    let entry = store.get_remote_journal(IssueId::new(3), JournalId::new(10));
    assert_eq!(entry.journal.notes, "updated notes");
    assert!(matches!(entry.state, RemoteJournalState::Synced));
}

#[test]
fn sync_fetched_drops_synced_journals_missing_from_the_fetched_result() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(3),
            IssueJournals {
                remote: vec![
                    remote_entry(IssueId::new(3), JournalId::new(10)),
                    remote_entry(IssueId::new(3), JournalId::new(11)),
                ],
                local: Some(local_entry(IssueId::new(3))),
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![journal(IssueId::new(3), JournalId::new(10))],
    });

    let journals = store.get_remote_journals(IssueId::new(3));
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(10)]
    );
    assert!(matches!(
        store
            .get_local_journal(IssueId::new(3))
            .expect("local journal should be kept")
            .state,
        LocalJournalState::Uploading
    ));
}

#[test]
fn sync_fetched_appends_dirty_journals_missing_from_the_fetched_result_in_their_previous_relative_order()
 {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(3),
            IssueJournals {
                remote: vec![
                    remote_entry(IssueId::new(3), JournalId::new(10)),
                    edited_remote_entry(IssueId::new(3), JournalId::new(11)),
                    uploading_remote_entry(IssueId::new(3), JournalId::new(12)),
                    remote_entry(IssueId::new(3), JournalId::new(13)),
                    edited_remote_entry(IssueId::new(3), JournalId::new(14)),
                ],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![
            journal(IssueId::new(3), JournalId::new(10)),
            journal(IssueId::new(3), JournalId::new(15)),
        ],
    });

    let journals = store.get_remote_journals(IssueId::new(3));
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![
            JournalId::new(10),
            JournalId::new(15),
            JournalId::new(11),
            JournalId::new(12),
            JournalId::new(14),
        ]
    );

    let kept_edited = store.get_remote_journal(IssueId::new(3), JournalId::new(11));
    assert!(matches!(
        kept_edited.state,
        RemoteJournalState::Edited { .. }
    ));
    assert_eq!(kept_edited.journal.notes, "remote notes 11");

    let kept_uploading = store.get_remote_journal(IssueId::new(3), JournalId::new(12));
    assert!(matches!(
        kept_uploading.state,
        RemoteJournalState::Uploading { .. }
    ));
    assert_eq!(kept_uploading.journal.notes, "remote notes 12");
}

#[test]
fn sync_fetched_keeps_a_dirty_journal_present_in_the_fetched_result_in_the_fetched_position() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(3),
            IssueJournals {
                remote: vec![
                    edited_remote_entry(IssueId::new(3), JournalId::new(10)),
                    remote_entry(IssueId::new(3), JournalId::new(11)),
                ],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![
            journal_with_notes(IssueId::new(3), JournalId::new(11), "updated notes"),
            journal(IssueId::new(3), JournalId::new(10)),
        ],
    });

    let journals = store.get_remote_journals(IssueId::new(3));
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(11), JournalId::new(10)]
    );
    assert!(matches!(
        store
            .get_remote_journal(IssueId::new(3), JournalId::new(10))
            .state,
        RemoteJournalState::Edited { .. }
    ));
    assert_eq!(
        store
            .get_remote_journal(IssueId::new(3), JournalId::new(11))
            .journal
            .notes,
        "updated notes"
    );
}

#[test]
fn has_remote_journal_is_false_for_another_issue_or_journal_id() {
    let store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    assert!(!store.has_remote_journal(IssueId::new(2), JournalId::new(10)));
    assert!(!store.has_remote_journal(IssueId::new(1), JournalId::new(11)));
}

#[test]
#[should_panic(expected = "remote journal 10 is not registered for issue 1")]
fn get_remote_journal_panics_when_the_journal_is_missing() {
    let store = JournalStore::new();

    store.get_remote_journal(1, 10);
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn get_remote_journal_panics_when_only_another_journal_of_the_issue_exists() {
    let store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.get_remote_journal(1, 11);
}

#[test]
fn edit_remote_notes_moves_a_synced_journal_to_edited_with_the_notes_as_before() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::EditRemoteNotes {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
        notes: "edited notes".to_string(),
    });

    let entry = store.get_remote_journal(IssueId::new(1), JournalId::new(10));
    let RemoteJournalState::Edited { diff, failure } = &entry.state else {
        panic!("expected edited state");
    };
    assert_eq!(diff.before, "remote notes 10");
    assert_eq!(diff.after, "edited notes");
    assert!(failure.is_none());
}

#[test]
fn edit_remote_notes_keeps_the_first_before_and_updates_the_after_on_repeated_edits() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![edited_remote_entry_with_failure(
                    issue_id,
                    JournalId::new(10),
                    "first before",
                    "first after",
                )],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::EditRemoteNotes {
        issue_id,
        journal_id: JournalId::new(10),
        notes: "second after".to_string(),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    let RemoteJournalState::Edited { diff, failure } = &entry.state else {
        panic!("expected edited state");
    };
    assert_eq!(diff.before, "first before");
    assert_eq!(diff.after, "second after");
    assert!(failure.is_none());
}

#[test]
fn edit_remote_notes_returns_to_synced_when_the_notes_match_the_before_value() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![edited_remote_entry_with_failure(
                    issue_id,
                    JournalId::new(10),
                    "first before",
                    "first after",
                )],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::EditRemoteNotes {
        issue_id,
        journal_id: JournalId::new(10),
        notes: "first before".to_string(),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    assert!(matches!(entry.state, RemoteJournalState::Synced));
}

#[test]
fn edit_remote_notes_from_synced_with_identical_notes_keeps_the_journal_synced() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![remote_entry(issue_id, JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::EditRemoteNotes {
        issue_id,
        journal_id: JournalId::new(10),
        notes: "remote notes 10".to_string(),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    assert!(matches!(entry.state, RemoteJournalState::Synced));
}

#[test]
#[should_panic(expected = "cannot edit remote journal 10 while it is uploading")]
fn edit_remote_notes_panics_while_the_journal_is_uploading() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![uploading_remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::EditRemoteNotes {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
        notes: "edited notes".to_string(),
    });
}

#[test]
fn start_remote_upload_moves_an_edited_journal_to_uploading_without_changing_the_diff() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![edited_remote_entry(issue_id, JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::StartRemoteUpload {
        issue_id,
        journal_id: JournalId::new(10),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    let RemoteJournalState::Uploading { diff, conflict } = &entry.state else {
        panic!("expected uploading state");
    };
    assert_eq!(diff.before, "remote notes 10");
    assert_eq!(diff.after, "edited notes");
    assert!(conflict.is_none());
}

#[test]
fn start_remote_upload_drops_a_kept_upload_failure_and_keeps_the_diff() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![edited_remote_entry_with_failure(
                    issue_id,
                    JournalId::new(10),
                    "first before",
                    "first after",
                )],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::StartRemoteUpload {
        issue_id,
        journal_id: JournalId::new(10),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    let RemoteJournalState::Uploading { diff, conflict } = &entry.state else {
        panic!("expected uploading state");
    };
    assert_eq!(diff.before, "first before");
    assert_eq!(diff.after, "first after");
    assert!(conflict.is_none());
}

#[test]
#[should_panic(expected = "cannot start remote journal upload while it is synced")]
fn start_remote_upload_panics_when_the_journal_is_synced() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::StartRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
    });
}

#[test]
#[should_panic(expected = "cannot start remote journal upload while it is uploading")]
fn start_remote_upload_panics_when_the_journal_is_already_uploading() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![uploading_remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::StartRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
    });
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn start_remote_upload_panics_when_the_journal_is_not_registered_for_the_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::StartRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(11),
    });
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn edit_remote_notes_panics_when_the_journal_is_not_registered_for_the_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::EditRemoteNotes {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(11),
        notes: "edited notes".to_string(),
    });
}

#[test]
fn fail_remote_upload_moves_an_uploading_journal_to_edited_with_the_failure_message() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![uploading_remote_entry(issue_id, JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::FailRemoteUpload {
        issue_id,
        journal_id: JournalId::new(10),
        message: "network error: offline".to_string(),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    let RemoteJournalState::Edited { diff, failure } = &entry.state else {
        panic!("expected edited state");
    };
    assert_eq!(diff.before, "remote notes 10");
    assert_eq!(diff.after, "uploading notes");
    let Some(failure) = failure else {
        panic!("expected failure")
    };
    assert_eq!(failure.message.as_str(), "network error: offline");
}

#[test]
#[should_panic(expected = "cannot fail remote journal 10 upload while it is synced")]
fn fail_remote_upload_panics_when_the_journal_is_synced() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::FailRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
        message: "network error: offline".to_string(),
    });
}

#[test]
#[should_panic(expected = "cannot fail remote journal 10 upload while it is edited")]
fn fail_remote_upload_panics_when_the_journal_is_edited() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![edited_remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::FailRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
        message: "network error: offline".to_string(),
    });
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn fail_remote_upload_panics_when_the_journal_is_not_registered_for_the_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::FailRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(11),
        message: "network error: offline".to_string(),
    });
}

#[test]
fn complete_remote_upload_moves_an_uploading_journal_to_synced_with_the_notes_and_keeps_updated_on()
{
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![uploading_remote_entry(issue_id, JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::CompleteRemoteUpload {
        issue_id,
        journal_id: JournalId::new(10),
        notes: "edited notes".to_string(),
    });

    let entry = store.get_remote_journal(issue_id, JournalId::new(10));
    assert!(matches!(entry.state, RemoteJournalState::Synced));
    assert_eq!(entry.journal.notes, "edited notes");
    assert_eq!(
        entry.journal.updated_on,
        local_datetime("2026-09-10T00:00:00+09:00")
    );
}

#[test]
#[should_panic(expected = "cannot complete remote journal 10 upload while it is synced")]
fn complete_remote_upload_panics_when_the_journal_is_synced() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::CompleteRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
        notes: "edited notes".to_string(),
    });
}

#[test]
#[should_panic(expected = "cannot complete remote journal 10 upload while it is edited")]
fn complete_remote_upload_panics_when_the_journal_is_edited() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![edited_remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::CompleteRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
        notes: "edited notes".to_string(),
    });
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn complete_remote_upload_panics_when_the_journal_is_not_registered_for_the_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::CompleteRemoteUpload {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(11),
        notes: "edited notes".to_string(),
    });
}

#[test]
fn remove_missing_remote_journal_removes_an_uploading_entry_and_keeps_the_rest() {
    let issue_id = IssueId::new(1);
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            issue_id,
            IssueJournals {
                remote: vec![
                    uploading_remote_entry(issue_id, JournalId::new(10)),
                    remote_entry(issue_id, JournalId::new(11)),
                ],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::RemoveMissingRemoteJournal {
        issue_id,
        journal_id: JournalId::new(10),
    });

    let journals = store.get_remote_journals(issue_id);
    assert_eq!(
        journals
            .iter()
            .map(|entry| entry.journal.id)
            .collect::<Vec<_>>(),
        vec![JournalId::new(11)]
    );
}

#[test]
#[should_panic(expected = "cannot remove remote journal 10 while it is synced")]
fn remove_missing_remote_journal_panics_when_the_journal_is_synced() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::RemoveMissingRemoteJournal {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
    });
}

#[test]
#[should_panic(expected = "cannot remove remote journal 10 while it is edited")]
fn remove_missing_remote_journal_panics_when_the_journal_is_edited() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![edited_remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::RemoveMissingRemoteJournal {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(10),
    });
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn remove_missing_remote_journal_panics_when_the_journal_is_not_registered_for_the_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: None,
            },
        )]),
    };

    store.consume_action(JournalAction::RemoveMissingRemoteJournal {
        issue_id: IssueId::new(1),
        journal_id: JournalId::new(11),
    });
}

fn edited_remote_entry_with_failure(
    issue_id: impl Into<IssueId>,
    journal_id: impl Into<JournalId>,
    before: &str,
    after: &str,
) -> RemoteJournalEntry {
    let journal_id = journal_id.into();
    RemoteJournalEntry {
        journal: journal(issue_id, journal_id),
        state: RemoteJournalState::Edited {
            diff: JournalNotesDiff {
                before: before.to_string(),
                after: after.to_string(),
            },
            failure: Some(JournalUploadFailure {
                message: "upload failed".to_string(),
            }),
        },
    }
}

#[test]
#[should_panic(expected = "sync fetched journals for issue 3 contain duplicate journal 10")]
fn sync_fetched_panics_when_the_fetched_result_contains_duplicate_journals() {
    let mut store = JournalStore::new();

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![
            journal(IssueId::new(3), JournalId::new(10)),
            journal(IssueId::new(3), JournalId::new(10)),
        ],
    });
}

#[test]
#[should_panic(expected = "sync fetched journal 10 of issue 3 has issue 4")]
fn sync_fetched_panics_when_a_fetched_journal_belongs_to_another_issue() {
    let mut store = JournalStore::new();

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![journal(IssueId::new(4), JournalId::new(10))],
    });
}

#[test]
#[should_panic(expected = "remote journal 10 is already registered for issue 1")]
fn sync_fetched_panics_when_a_fetched_journal_id_is_registered_for_another_issue() {
    let mut store = JournalStore {
        by_issue: HashMap::from([(
            IssueId::new(1),
            IssueJournals {
                remote: vec![remote_entry(IssueId::new(1), JournalId::new(10))],
                local: Some(local_entry(IssueId::new(1))),
            },
        )]),
    };

    store.consume_action(JournalAction::SyncFetched {
        issue_id: IssueId::new(3),
        journals: vec![journal(IssueId::new(3), JournalId::new(10))],
    });
}
