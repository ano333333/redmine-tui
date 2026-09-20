use super::{
    JournalUploadFailure, LocalJournalEntry, LocalJournalState, RemoteJournalEntry,
    RemoteJournalState, RemoteJournalUploadConflict,
};
use crate::entities::{Journal, LocalJournal};
use crate::test_support::local_datetime;
use crate::vos::{IssueId, JournalId, JournalNotesDiff};

#[test]
fn remote_journal_state_holds_its_associated_values_per_variant() {
    let diff = JournalNotesDiff {
        before: "old notes".to_string(),
        after: "new notes".to_string(),
    };
    let failure = JournalUploadFailure {
        message: "PUT failed".to_string(),
    };
    let conflict = RemoteJournalUploadConflict {
        server_notes: "server notes".to_string(),
    };

    let synced = RemoteJournalState::Synced;
    let edited = RemoteJournalState::Edited {
        diff: diff.clone(),
        failure: Some(failure.clone()),
    };
    let uploading = RemoteJournalState::Uploading {
        diff: diff.clone(),
        conflict: Some(conflict.clone()),
    };

    assert!(matches!(synced, RemoteJournalState::Synced));

    let RemoteJournalState::Edited {
        diff: edited_diff,
        failure: edited_failure,
    } = &edited
    else {
        panic!("edited state should hold diff and failure");
    };
    assert_eq!(edited_diff, &diff);
    assert_eq!(edited_failure.as_ref(), Some(&failure));

    let RemoteJournalState::Uploading {
        diff: uploading_diff,
        conflict: uploading_conflict,
    } = &uploading
    else {
        panic!("uploading state should hold diff and conflict");
    };
    assert_eq!(uploading_diff, &diff);
    assert_eq!(uploading_conflict.as_ref(), Some(&conflict));
}

#[test]
fn local_journal_state_holds_optional_failure_in_local_only() {
    let failure = JournalUploadFailure {
        message: "GET failed".to_string(),
    };

    let local_only = LocalJournalState::LocalOnly {
        failure: Some(failure.clone()),
    };
    let uploading = LocalJournalState::Uploading;

    let LocalJournalState::LocalOnly {
        failure: local_failure,
    } = &local_only
    else {
        panic!("local only state should hold failure");
    };
    assert_eq!(local_failure.as_ref(), Some(&failure));

    assert!(matches!(uploading, LocalJournalState::Uploading));
}

#[test]
fn journal_entries_wrap_entity_and_state() {
    let journal = Journal {
        id: JournalId::new(10),
        issue_id: IssueId::new(1),
        user: "admin".to_string(),
        updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
        details: vec![],
        notes: "remote notes".to_string(),
    };
    let remote_entry = RemoteJournalEntry {
        journal: journal.clone(),
        state: RemoteJournalState::Synced,
    };

    let local = LocalJournal {
        issue_id: IssueId::new(1),
        notes: "local notes".to_string(),
    };
    let local_entry = LocalJournalEntry {
        journal: local.clone(),
        state: LocalJournalState::Uploading,
    };

    assert_eq!(remote_entry.journal.id, journal.id);
    assert_eq!(remote_entry.journal.notes, journal.notes);
    assert!(matches!(remote_entry.state, RemoteJournalState::Synced));

    assert_eq!(local_entry.journal.issue_id, local.issue_id);
    assert_eq!(local_entry.journal.notes, local.notes);
    assert!(matches!(local_entry.state, LocalJournalState::Uploading));
}
