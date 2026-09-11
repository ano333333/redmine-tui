use std::collections::HashMap;

use super::journal_store::{IssueJournals, JournalAction, JournalStore};
use crate::entities::{Journal, LocalJournal};
use crate::stores::journal_state::{
    LocalJournalEntry, LocalJournalState, RemoteJournalEntry, RemoteJournalState,
};
use crate::test_support::local_datetime;
use crate::vos::{IssueId, JournalId};

fn journal(issue_id: impl Into<IssueId>, journal_id: impl Into<JournalId>) -> Journal {
    let issue_id = issue_id.into();
    let journal_id = journal_id.into();
    Journal {
        id: journal_id,
        issue_id,
        user: "admin".to_string(),
        updated_on: local_datetime("2026-09-10T00:00:00+09:00"),
        details: vec![],
        notes: format!("remote notes {journal_id}"),
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
