use std::collections::HashMap;

use super::journal_store::{IssueJournals, JournalAction, JournalStore};
use crate::entities::{Journal, LocalJournal};
use crate::stores::journal_state::{
    LocalJournalEntry, LocalJournalState, RemoteJournalEntry, RemoteJournalState,
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
