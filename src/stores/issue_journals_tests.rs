use super::{Action, IssueAction, IssueState, JournalAction, Store};
use crate::entities::{IssueAggregate, IssueChild, Journal};
use crate::stores::journal_state::{
    DeletedJournalEntry, DeletedJournalState, JournalUploadFailure, LocalJournalState,
    RemoteJournalState, RemoteJournalUploadConflict,
};
use crate::test_support::{local_datetime, sample_issue_aggregate};
use crate::vos::{EntityIdValue, IssueId, JournalId, JournalNotesDiff};

const ISSUE_ID: u16 = 1;
const OTHER_ISSUE_ID: u16 = 2;

fn journal_of(issue_id: u16, journal_id: u16) -> Journal {
    Journal {
        id: JournalId::new(journal_id),
        issue_id: IssueId::new(issue_id),
        user: "admin".to_string(),
        updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
        details: vec![],
        notes: format!("remote notes {journal_id}"),
    }
}

fn journal(journal_id: u16) -> Journal {
    journal_of(ISSUE_ID, journal_id)
}

fn journal_with_notes(journal_id: u16, notes: &str) -> Journal {
    Journal {
        notes: notes.to_string(),
        ..journal(journal_id)
    }
}

fn issue_with_journals(issue_id: u16, journals: Vec<Journal>) -> IssueAggregate {
    let mut issue = sample_issue_aggregate(issue_id, "issue", 1.into(), None, None, None, 0);
    issue.journals = journals;
    issue
}

fn store_with(journal_ids: &[u16]) -> Store {
    let mut store = Store::new();
    store.consume_action(
        IssueAction::Sync {
            issue: issue_with_journals(
                ISSUE_ID,
                journal_ids.iter().copied().map(journal).collect(),
            ),
        }
        .into(),
    );
    store
}

fn apply(store: &mut Store, action: JournalAction) {
    store.consume_action(Action::Journal(action));
}

fn edit_remote(store: &mut Store, journal_id: u16, notes: &str) {
    apply(
        store,
        JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID.into(),
            journal_id: journal_id.into(),
            notes: notes.to_string(),
        },
    );
}

fn start_remote(store: &mut Store, journal_id: u16) {
    apply(
        store,
        JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: journal_id.into(),
        },
    );
}

fn detect_conflict(store: &mut Store, journal_id: u16, server_notes: &str) {
    apply(
        store,
        JournalAction::DetectRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: journal_id.into(),
            server_notes: server_notes.to_string(),
        },
    );
}

/// Issue属性の保存成功として、取得したJournalを含むIssueを取り込む。
fn take_in_fetched_journals(store: &mut Store, journals: Vec<Journal>) {
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "saved body".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::Sync {
            issue: issue_with_journals(ISSUE_ID, journals),
        }
        .into(),
    );
}

fn create_local(store: &mut Store) {
    apply(
        store,
        JournalAction::CreateLocal {
            issue_id: ISSUE_ID.into(),
        },
    );
}

fn start_local(store: &mut Store) {
    apply(
        store,
        JournalAction::StartLocalUpload {
            issue_id: ISSUE_ID.into(),
        },
    );
}

/// 指定したJournalを"edited notes"へ編集したStore。
fn edited_store(journal_ids: &[u16], edited_id: u16) -> Store {
    let mut store = store_with(journal_ids);
    edit_remote(&mut store, edited_id, "edited notes");
    store
}

fn uploading_store(journal_ids: &[u16], uploading_id: u16) -> Store {
    let mut store = edited_store(journal_ids, uploading_id);
    start_remote(&mut store, uploading_id);
    store
}

fn conflicted_store(journal_ids: &[u16], conflicted_id: u16) -> Store {
    let mut store = uploading_store(journal_ids, conflicted_id);
    detect_conflict(&mut store, conflicted_id, "server notes");
    store
}

fn state(store: &Store, journal_id: u16) -> RemoteJournalState {
    store.get_remote_journal(ISSUE_ID, journal_id).state.clone()
}

fn journal_ids(store: &Store) -> Vec<u16> {
    store
        .get_remote_journals(ISSUE_ID)
        .iter()
        .map(|entry| entry.journal.id.get())
        .collect()
}

fn edited_state(before: &str, after: &str) -> RemoteJournalState {
    RemoteJournalState::Edited {
        diff: JournalNotesDiff {
            before: before.to_string(),
            after: after.to_string(),
        },
        failure: None,
    }
}

fn uploading_state(before: &str, after: &str) -> RemoteJournalState {
    RemoteJournalState::Uploading {
        diff: JournalNotesDiff {
            before: before.to_string(),
            after: after.to_string(),
        },
        conflict: None,
    }
}

#[test]
fn getters_for_an_unloaded_issue_return_empty_results() {
    let store = Store::new();

    assert!(store.get_remote_journals(ISSUE_ID).is_empty());
    assert!(store.try_get_local_journal(ISSUE_ID).is_none());
    assert!(!store.has_uploading_journal(ISSUE_ID));
}

#[test]
fn every_journal_of_a_synced_issue_is_listed_as_synced_in_the_fetched_order() {
    let store = store_with(&[11, 10]);

    assert_eq!(journal_ids(&store), vec![11, 10]);
    let journal = store.get_remote_journal(ISSUE_ID, 11);
    assert_eq!(journal.journal.notes, "remote notes 11");
    assert_eq!(*journal.state, RemoteJournalState::Synced);
    assert_eq!(state(&store, 10), RemoteJournalState::Synced);
}

#[test]
#[should_panic(expected = "cannot update journals of issue 1 while it is Unregistered")]
fn journal_actions_panic_while_the_issue_is_not_loaded() {
    let mut store = Store::new();

    create_local(&mut store);
}

#[test]
#[should_panic(expected = "cannot update journals of issue 1 while it is Fetching")]
fn journal_actions_panic_while_the_issue_is_fetching() {
    let mut store = Store::new();
    store.consume_action(
        IssueAction::StartFetching {
            id: ISSUE_ID.into(),
        }
        .into(),
    );

    create_local(&mut store);
}

#[test]
fn has_uploading_journal_detects_remote_and_local_uploads_of_the_issue_only() {
    let remote_uploading = uploading_store(&[10], 10);
    let mut local_uploading = store_with(&[]);
    create_local(&mut local_uploading);
    start_local(&mut local_uploading);
    let idle = edited_store(&[10], 10);

    assert!(remote_uploading.has_uploading_journal(ISSUE_ID));
    assert!(local_uploading.has_uploading_journal(ISSUE_ID));
    assert!(!idle.has_uploading_journal(ISSUE_ID));
    assert!(!remote_uploading.has_uploading_journal(OTHER_ISSUE_ID));
}

#[test]
fn journal_states_are_independent_of_the_issue_state() {
    let store = edited_store(&[10], 10);

    assert_eq!(
        store.try_get_issue_state(ISSUE_ID),
        Some(IssueState::Synced)
    );
    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
}

#[test]
fn issue_transitions_keep_journal_edits_and_the_local_draft() {
    let mut store = edited_store(&[10], 10);
    create_local(&mut store);
    apply(
        &mut store,
        JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID.into(),
            notes: "draft".to_string(),
        },
    );

    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::UploadSucceeded {
            issue: issue_with_journals(ISSUE_ID, vec![journal_with_notes(10, "remote notes 10")]),
            children: vec![],
        }
        .into(),
    );

    assert_eq!(
        store.try_get_issue_state(ISSUE_ID),
        Some(IssueState::Synced)
    );
    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
    assert_eq!(store.get_local_journal(ISSUE_ID).journal.notes, "draft");
}

#[test]
fn sync_of_a_saved_issue_takes_in_fetched_journals_and_evacuates_missing_edits() {
    let mut store = edited_store(&[10, 11], 10);
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );

    store.consume_action(
        IssueAction::Sync {
            issue: issue_with_journals(
                ISSUE_ID,
                vec![journal_with_notes(11, "updated notes"), journal(12)],
            ),
        }
        .into(),
    );

    assert_eq!(journal_ids(&store), vec![11, 12]);
    assert_eq!(
        store.get_remote_journal(ISSUE_ID, 11).journal.notes,
        "updated notes"
    );
    assert_eq!(state(&store, 12), RemoteJournalState::Synced);
    assert_eq!(deleted_journals(&store), vec![deleted(10, "edited notes")]);
}

#[test]
#[should_panic(expected = "cannot start issue upload while a journal of issue 1 is uploading")]
fn issue_upload_start_panics_while_a_journal_of_the_issue_is_uploading() {
    let mut store = uploading_store(&[10], 10);
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );

    store.consume_action(
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        }
        .into(),
    );
}

#[test]
#[should_panic(expected = "cannot start local journal upload while issue 1 is uploading")]
fn local_journal_upload_start_panics_while_the_issue_is_uploading() {
    let mut store = store_with(&[]);
    create_local(&mut store);
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        }
        .into(),
    );

    start_local(&mut store);
}

#[test]
#[should_panic(expected = "cannot start remote journal upload while issue 1 is uploading")]
fn remote_journal_upload_start_panics_while_the_issue_is_uploading() {
    let mut store = edited_store(&[10], 10);
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        }
        .into(),
    );

    start_remote(&mut store, 10);
}

#[test]
fn fetched_journals_replaces_synced_journals_and_keeps_the_fetched_order() {
    let mut store = store_with(&[10, 11]);

    take_in_fetched_journals(
        &mut store,
        vec![journal(12), journal_with_notes(10, "updated notes")],
    );

    assert_eq!(journal_ids(&store), vec![12, 10]);
    assert_eq!(
        store.get_remote_journal(ISSUE_ID, 10).journal.notes,
        "updated notes"
    );
    assert_eq!(state(&store, 12), RemoteJournalState::Synced);
}

fn deleted(original_id: u16, notes: &str) -> DeletedJournalEntry {
    DeletedJournalEntry {
        original_id: JournalId::new(original_id),
        notes: notes.to_string(),
        state: DeletedJournalState::Pending { failure: None },
    }
}

fn deleted_journals(store: &Store) -> Vec<DeletedJournalEntry> {
    store.get_deleted_journals(ISSUE_ID).to_vec()
}

fn start_deleted(store: &mut Store, original_id: u16) {
    apply(
        store,
        JournalAction::StartDeletedUpload {
            issue_id: ISSUE_ID.into(),
            original_id: original_id.into(),
        },
    );
}

/// Journal 10を編集した後、取得結果から消えて退避されたStore。
fn store_with_deleted_10() -> Store {
    let mut store = edited_store(&[10, 11], 10);
    take_in_fetched_journals(&mut store, vec![journal(11)]);
    store
}

#[test]
fn fetched_journals_evacuate_missing_edited_journals_in_order_and_keep_missing_uploading_ones() {
    let mut store = store_with(&[10, 11, 12, 13, 14]);
    edit_remote(&mut store, 11, "edited 11");
    edit_remote(&mut store, 12, "uploading notes");
    start_remote(&mut store, 12);
    edit_remote(&mut store, 14, "edited 14");

    take_in_fetched_journals(&mut store, vec![journal(10), journal(15)]);

    assert_eq!(journal_ids(&store), vec![10, 15, 12]);
    assert_eq!(
        state(&store, 12),
        uploading_state("remote notes 12", "uploading notes")
    );
    assert_eq!(
        deleted_journals(&store),
        vec![deleted(11, "edited 11"), deleted(14, "edited 14")]
    );
}

#[test]
fn an_evacuated_journal_is_not_evacuated_twice_when_it_is_still_missing() {
    let mut store = store_with_deleted_10();

    take_in_fetched_journals(&mut store, vec![journal(11)]);

    assert_eq!(deleted_journals(&store), vec![deleted(10, "edited notes")]);
}

#[test]
fn a_reappearing_journal_is_restored_as_an_edit_from_the_server_notes() {
    let mut store = store_with_deleted_10();

    take_in_fetched_journals(
        &mut store,
        vec![journal_with_notes(10, "server notes"), journal(11)],
    );

    assert!(deleted_journals(&store).is_empty());
    assert_eq!(journal_ids(&store), vec![10, 11]);
    assert_eq!(
        state(&store, 10),
        edited_state("server notes", "edited notes")
    );
}

#[test]
fn a_reappearing_journal_with_the_same_notes_is_restored_as_synced() {
    let mut store = store_with_deleted_10();

    take_in_fetched_journals(
        &mut store,
        vec![journal_with_notes(10, "edited notes"), journal(11)],
    );

    assert!(deleted_journals(&store).is_empty());
    assert_eq!(state(&store, 10), RemoteJournalState::Synced);
}

#[test]
fn evacuation_keeps_the_local_journal() {
    let mut store = edited_store(&[10], 10);
    create_local(&mut store);
    apply(
        &mut store,
        JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID.into(),
            notes: "draft".to_string(),
        },
    );

    take_in_fetched_journals(&mut store, vec![]);

    assert_eq!(deleted_journals(&store), vec![deleted(10, "edited notes")]);
    assert_eq!(store.get_local_journal(ISSUE_ID).journal.notes, "draft");
}

#[test]
fn edit_deleted_notes_replaces_the_notes_and_drops_a_failure() {
    let mut store = store_with_deleted_10();
    start_deleted(&mut store, 10);
    apply(
        &mut store,
        JournalAction::FailDeletedUpload {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
            message: "network error".to_string(),
        },
    );

    apply(
        &mut store,
        JournalAction::EditDeletedNotes {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
            notes: "rewritten".to_string(),
        },
    );

    assert_eq!(deleted_journals(&store), vec![deleted(10, "rewritten")]);
}

#[test]
fn fail_deleted_upload_keeps_the_notes_with_the_failure() {
    let mut store = store_with_deleted_10();
    start_deleted(&mut store, 10);

    apply(
        &mut store,
        JournalAction::FailDeletedUpload {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
            message: "network error".to_string(),
        },
    );

    assert_eq!(
        deleted_journals(&store),
        vec![DeletedJournalEntry {
            state: DeletedJournalState::Pending {
                failure: Some(JournalUploadFailure {
                    message: "network error".to_string(),
                }),
            },
            ..deleted(10, "edited notes")
        }]
    );
}

#[test]
fn complete_deleted_upload_with_fetched_removes_only_the_target_and_takes_in_the_new_journal() {
    let mut store = edited_store(&[10, 11, 12], 10);
    edit_remote(&mut store, 11, "edited 11");
    take_in_fetched_journals(&mut store, vec![journal(12)]);
    create_local(&mut store);
    start_deleted(&mut store, 10);

    apply(
        &mut store,
        JournalAction::CompleteDeletedUploadWithFetched {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
            journals: vec![journal(12), journal_with_notes(13, "edited notes")],
        },
    );

    assert_eq!(deleted_journals(&store), vec![deleted(11, "edited 11")]);
    assert_eq!(journal_ids(&store), vec![12, 13]);
    assert!(store.try_get_local_journal(ISSUE_ID).is_some());
}

#[test]
fn complete_deleted_upload_without_fetch_removes_only_the_target() {
    let mut store = edited_store(&[10, 11], 10);
    edit_remote(&mut store, 11, "edited 11");
    take_in_fetched_journals(&mut store, vec![]);
    start_deleted(&mut store, 10);

    apply(
        &mut store,
        JournalAction::CompleteDeletedUploadWithoutFetch {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
        },
    );

    assert_eq!(deleted_journals(&store), vec![deleted(11, "edited 11")]);
}

#[test]
fn discard_deleted_removes_the_target() {
    let mut store = store_with_deleted_10();

    apply(
        &mut store,
        JournalAction::DiscardDeleted {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
        },
    );

    assert!(deleted_journals(&store).is_empty());
}

#[test]
fn has_uploading_journal_detects_an_uploading_deleted_journal() {
    let mut store = store_with_deleted_10();

    start_deleted(&mut store, 10);

    assert!(store.has_uploading_journal(ISSUE_ID));
}

#[test]
#[should_panic(
    expected = "cannot start deleted journal upload while another journal of issue 1 is uploading"
)]
fn start_deleted_upload_panics_while_the_local_journal_is_uploading() {
    let mut store = store_with_deleted_10();
    create_local(&mut store);
    start_local(&mut store);

    start_deleted(&mut store, 10);
}

#[test]
#[should_panic(expected = "cannot start deleted journal upload while issue 1 is uploading")]
fn start_deleted_upload_panics_while_the_issue_is_uploading() {
    let mut store = store_with_deleted_10();
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        }
        .into(),
    );

    start_deleted(&mut store, 10);
}

#[test]
#[should_panic(expected = "cannot discard deleted journal 10 of issue 1 while it is uploading")]
fn discard_deleted_panics_while_it_is_uploading() {
    let mut store = store_with_deleted_10();
    start_deleted(&mut store, 10);

    apply(
        &mut store,
        JournalAction::DiscardDeleted {
            issue_id: ISSUE_ID.into(),
            original_id: 10.into(),
        },
    );
}

#[test]
#[should_panic(expected = "deleted journal 12 is not registered for issue 1")]
fn deleted_journal_actions_panic_for_an_unknown_original_id() {
    let mut store = store_with_deleted_10();

    start_deleted(&mut store, 12);
}

#[test]
fn fetched_journals_update_the_body_of_an_edited_journal_and_keep_its_diff() {
    let mut store = edited_store(&[10, 11], 10);

    take_in_fetched_journals(
        &mut store,
        vec![
            journal_with_notes(11, "updated notes"),
            journal_with_notes(10, "server edited notes"),
        ],
    );

    assert_eq!(journal_ids(&store), vec![11, 10]);
    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
    assert_eq!(
        store.get_remote_journal(ISSUE_ID, 10).journal.notes,
        "server edited notes"
    );
    assert_eq!(
        store.get_remote_journal(ISSUE_ID, 11).journal.notes,
        "updated notes"
    );
}

#[test]
fn fetched_journals_sync_an_edited_journal_whose_edit_is_already_on_the_server() {
    let mut store = edited_store(&[10], 10);

    take_in_fetched_journals(&mut store, vec![journal_with_notes(10, "edited notes")]);

    assert_eq!(state(&store, 10), RemoteJournalState::Synced);
    assert_eq!(
        store.get_remote_journal(ISSUE_ID, 10).journal.notes,
        "edited notes"
    );
}

#[test]
fn fetched_journals_keeps_the_local_journal() {
    let mut store = store_with(&[]);
    create_local(&mut store);
    start_local(&mut store);

    take_in_fetched_journals(&mut store, vec![journal(10)]);

    assert_eq!(
        store.get_local_journal(ISSUE_ID).state,
        LocalJournalState::Uploading
    );
}

#[test]
#[should_panic(expected = "fetched journals for issue 1 contain duplicate journal 10")]
fn fetched_journals_panics_when_the_fetched_result_contains_duplicate_journals() {
    let mut store = store_with(&[]);

    take_in_fetched_journals(&mut store, vec![journal(10), journal(10)]);
}

#[test]
#[should_panic(expected = "fetched journal 10 of issue 1 has issue 2")]
fn fetched_journals_panics_when_a_fetched_journal_belongs_to_another_issue() {
    let mut store = store_with(&[]);

    take_in_fetched_journals(&mut store, vec![journal_of(OTHER_ISSUE_ID, 10)]);
}

#[test]
#[should_panic(expected = "remote journal 10 is already registered for issue 2")]
fn fetched_journals_panics_when_a_fetched_journal_id_is_registered_for_another_issue() {
    let mut store = store_with(&[]);
    store.consume_action(
        IssueAction::Sync {
            issue: issue_with_journals(OTHER_ISSUE_ID, vec![journal_of(OTHER_ISSUE_ID, 10)]),
        }
        .into(),
    );

    take_in_fetched_journals(&mut store, vec![journal(10)]);
}

#[test]
fn get_remote_journal_upload_conflict_returns_the_diff_and_conflict_while_conflicted() {
    let store = conflicted_store(&[10], 10);

    let (diff, conflict) = store
        .try_get_remote_journal_upload_conflict(ISSUE_ID, 10)
        .expect("conflict is retained");

    assert_eq!(
        diff,
        &JournalNotesDiff {
            before: "remote notes 10".to_string(),
            after: "edited notes".to_string(),
        }
    );
    assert_eq!(
        conflict,
        &RemoteJournalUploadConflict {
            server_notes: "server notes".to_string(),
        }
    );
}

#[test]
fn get_remote_journal_upload_conflict_is_none_without_a_conflict() {
    let uploading = uploading_store(&[10], 10);
    let edited = edited_store(&[10], 10);

    assert!(
        uploading
            .try_get_remote_journal_upload_conflict(ISSUE_ID, 10)
            .is_none()
    );
    assert!(
        edited
            .try_get_remote_journal_upload_conflict(ISSUE_ID, 10)
            .is_none()
    );
    assert!(
        edited
            .try_get_remote_journal_upload_conflict(ISSUE_ID, 11)
            .is_none()
    );
    assert!(
        Store::new()
            .try_get_remote_journal_upload_conflict(ISSUE_ID, 10)
            .is_none()
    );
}

#[test]
#[should_panic(expected = "remote journal 11 is not registered for issue 1")]
fn get_remote_journal_panics_when_the_journal_is_missing() {
    let store = store_with(&[10]);

    store.get_remote_journal(ISSUE_ID, 11);
}

#[test]
fn create_local_registers_an_empty_local_only_journal() {
    let mut store = store_with(&[]);

    create_local(&mut store);

    let local = store.get_local_journal(ISSUE_ID);
    assert_eq!(local.journal.issue_id, IssueId::new(ISSUE_ID));
    assert_eq!(local.journal.notes, "");
    assert_eq!(local.state, LocalJournalState::LocalOnly { failure: None });
}

#[test]
fn edit_local_notes_updates_a_local_only_journal_and_drops_a_failure() {
    let mut store = store_with(&[]);
    create_local(&mut store);
    start_local(&mut store);
    apply(
        &mut store,
        JournalAction::FailLocalUpload {
            issue_id: ISSUE_ID.into(),
            message: "network error".to_string(),
        },
    );

    apply(
        &mut store,
        JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID.into(),
            notes: "edited local notes".to_string(),
        },
    );

    let local = store.get_local_journal(ISSUE_ID);
    assert_eq!(local.journal.notes, "edited local notes");
    assert_eq!(local.state, LocalJournalState::LocalOnly { failure: None });
}

#[test]
#[should_panic(expected = "local journal is already registered for issue 1")]
fn create_local_panics_when_the_issue_already_has_a_local_journal() {
    let mut store = store_with(&[]);
    create_local(&mut store);

    create_local(&mut store);
}

#[test]
#[should_panic(expected = "cannot edit local journal for issue 1 while it is uploading")]
fn edit_local_notes_panics_while_the_journal_is_uploading() {
    let mut store = store_with(&[]);
    create_local(&mut store);
    start_local(&mut store);

    apply(
        &mut store,
        JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID.into(),
            notes: "late edit".to_string(),
        },
    );
}

#[test]
fn start_local_upload_moves_a_local_only_journal_to_uploading() {
    let mut store = store_with(&[]);
    create_local(&mut store);

    start_local(&mut store);

    assert_eq!(
        store.get_local_journal(ISSUE_ID).state,
        LocalJournalState::Uploading
    );
}

#[test]
#[should_panic(expected = "cannot start local journal upload for issue 1 while it is uploading")]
fn start_local_upload_panics_when_the_journal_is_uploading() {
    let mut store = store_with(&[]);
    create_local(&mut store);
    start_local(&mut store);

    start_local(&mut store);
}

#[test]
#[should_panic(
    expected = "cannot start local journal upload while another journal of issue 1 is uploading"
)]
fn start_local_upload_panics_when_a_remote_journal_is_uploading() {
    let mut store = uploading_store(&[10], 10);
    create_local(&mut store);

    start_local(&mut store);
}

#[test]
fn fail_local_upload_restores_local_only_with_failure_and_keeps_notes() {
    let mut store = store_with(&[]);
    create_local(&mut store);
    apply(
        &mut store,
        JournalAction::EditLocalNotes {
            issue_id: ISSUE_ID.into(),
            notes: "local notes".to_string(),
        },
    );
    start_local(&mut store);

    apply(
        &mut store,
        JournalAction::FailLocalUpload {
            issue_id: ISSUE_ID.into(),
            message: "network error".to_string(),
        },
    );

    let local = store.get_local_journal(ISSUE_ID);
    assert_eq!(local.journal.notes, "local notes");
    assert_eq!(
        local.state,
        LocalJournalState::LocalOnly {
            failure: Some(JournalUploadFailure {
                message: "network error".to_string(),
            }),
        }
    );
}

#[test]
#[should_panic(expected = "cannot fail local journal upload for issue 1 while it is local only")]
fn fail_local_upload_panics_when_the_journal_is_local_only() {
    let mut store = store_with(&[]);
    create_local(&mut store);

    apply(
        &mut store,
        JournalAction::FailLocalUpload {
            issue_id: ISSUE_ID.into(),
            message: "late failure".to_string(),
        },
    );
}

#[test]
fn edit_remote_notes_moves_a_synced_journal_to_edited_with_the_fetched_notes_as_before() {
    let mut store = store_with(&[10]);

    edit_remote(&mut store, 10, "edited notes");

    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
    assert_eq!(
        store.get_remote_journal(ISSUE_ID, 10).journal.notes,
        "remote notes 10"
    );
}

#[test]
fn edit_remote_notes_keeps_the_first_before_and_updates_the_after_on_repeated_edits() {
    let mut store = edited_store(&[10], 10);

    edit_remote(&mut store, 10, "second edit");

    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "second edit")
    );
}

#[test]
fn edit_remote_notes_returns_to_synced_when_the_notes_match_the_before_value() {
    let mut store = edited_store(&[10], 10);

    edit_remote(&mut store, 10, "remote notes 10");

    assert_eq!(state(&store, 10), RemoteJournalState::Synced);
}

#[test]
fn edit_remote_notes_from_synced_with_identical_notes_keeps_the_journal_synced() {
    let mut store = store_with(&[10]);

    edit_remote(&mut store, 10, "remote notes 10");

    assert_eq!(state(&store, 10), RemoteJournalState::Synced);
}

#[test]
fn edit_remote_notes_after_a_failure_drops_the_failure() {
    let mut store = uploading_store(&[10], 10);
    apply(
        &mut store,
        JournalAction::FailRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            message: "network error".to_string(),
        },
    );

    edit_remote(&mut store, 10, "second edit");

    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "second edit")
    );
}

#[test]
fn start_remote_upload_moves_an_edited_journal_to_uploading_without_changing_the_diff() {
    let store = uploading_store(&[10], 10);

    assert_eq!(
        state(&store, 10),
        uploading_state("remote notes 10", "edited notes")
    );
}

#[test]
fn start_remote_upload_drops_a_kept_upload_failure_and_keeps_the_diff() {
    let mut store = uploading_store(&[10], 10);
    apply(
        &mut store,
        JournalAction::FailRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            message: "network error".to_string(),
        },
    );

    start_remote(&mut store, 10);

    assert_eq!(
        state(&store, 10),
        uploading_state("remote notes 10", "edited notes")
    );
}

#[test]
#[should_panic(expected = "cannot start remote journal upload while it is synced")]
fn start_remote_upload_panics_when_the_journal_is_synced() {
    let mut store = store_with(&[10]);

    start_remote(&mut store, 10);
}

#[test]
#[should_panic(expected = "cannot start remote journal upload while it is uploading")]
fn start_remote_upload_panics_when_the_journal_is_already_uploading() {
    let mut store = uploading_store(&[10], 10);

    start_remote(&mut store, 10);
}

#[test]
#[should_panic(
    expected = "cannot start remote journal upload while another journal of issue 1 is uploading"
)]
fn start_remote_upload_panics_when_the_local_journal_is_uploading() {
    let mut store = edited_store(&[10], 10);
    create_local(&mut store);
    start_local(&mut store);

    start_remote(&mut store, 10);
}

#[test]
#[should_panic(
    expected = "cannot start remote journal upload while another journal of issue 1 is uploading"
)]
fn start_remote_upload_panics_when_another_remote_journal_is_uploading() {
    let mut store = uploading_store(&[10, 11], 10);
    edit_remote(&mut store, 11, "edited notes");

    start_remote(&mut store, 11);
}

#[test]
fn fail_remote_upload_moves_an_uploading_journal_to_edited_with_the_failure_message() {
    let mut store = uploading_store(&[10], 10);

    apply(
        &mut store,
        JournalAction::FailRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            message: "network error".to_string(),
        },
    );

    assert_eq!(
        state(&store, 10),
        RemoteJournalState::Edited {
            diff: JournalNotesDiff {
                before: "remote notes 10".to_string(),
                after: "edited notes".to_string(),
            },
            failure: Some(JournalUploadFailure {
                message: "network error".to_string(),
            }),
        }
    );
}

#[test]
fn detect_remote_upload_conflict_retains_the_conflict_and_keeps_the_diff() {
    let mut store = conflicted_store(&[10], 10);

    detect_conflict(&mut store, 10, "newer server notes");

    assert_eq!(
        state(&store, 10),
        RemoteJournalState::Uploading {
            diff: JournalNotesDiff {
                before: "remote notes 10".to_string(),
                after: "edited notes".to_string(),
            },
            conflict: Some(RemoteJournalUploadConflict {
                server_notes: "newer server notes".to_string(),
            }),
        }
    );
}

#[test]
fn cancel_remote_upload_conflict_returns_to_edited_with_the_diff_and_drops_the_conflict() {
    let mut store = conflicted_store(&[10], 10);

    apply(
        &mut store,
        JournalAction::CancelRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        },
    );

    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
}

#[test]
#[should_panic(
    expected = "cannot cancel remote journal 10 upload conflict while it is uploading without a conflict"
)]
fn cancel_remote_upload_conflict_panics_when_uploading_without_a_conflict() {
    let mut store = uploading_store(&[10], 10);

    apply(
        &mut store,
        JournalAction::CancelRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        },
    );
}

#[test]
fn complete_remote_upload_moves_an_uploading_journal_to_synced_with_the_notes_and_keeps_updated_on()
{
    let mut store = uploading_store(&[10], 10);

    apply(
        &mut store,
        JournalAction::CompleteRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            notes: "edited notes".to_string(),
        },
    );

    let completed = store.get_remote_journal(ISSUE_ID, 10);
    assert_eq!(*completed.state, RemoteJournalState::Synced);
    assert_eq!(completed.journal.notes, "edited notes");
    assert_eq!(
        completed.journal.updated_on,
        Some(local_datetime("2026-09-10T00:00:00+09:00"))
    );
}

#[test]
fn remove_missing_remote_journal_removes_an_uploading_entry_and_keeps_the_rest() {
    let mut store = uploading_store(&[10, 11], 10);

    apply(
        &mut store,
        JournalAction::RemoveMissingRemoteJournal {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        },
    );

    assert_eq!(journal_ids(&store), vec![11]);
}

#[test]
fn complete_local_upload_with_fetched_merges_the_remote_collection_and_clears_the_local_entry() {
    let mut store = store_with(&[10]);
    create_local(&mut store);
    start_local(&mut store);

    apply(
        &mut store,
        JournalAction::CompleteLocalUploadWithFetched {
            issue_id: ISSUE_ID.into(),
            journals: vec![journal(10), journal(11)],
        },
    );

    assert_eq!(journal_ids(&store), vec![10, 11]);
    assert_eq!(state(&store, 11), RemoteJournalState::Synced);
    assert!(store.try_get_local_journal(ISSUE_ID).is_none());
}

#[test]
fn complete_local_upload_with_fetched_keeps_the_diff_of_edited_remote_entries() {
    let mut store = edited_store(&[10], 10);
    create_local(&mut store);
    start_local(&mut store);

    apply(
        &mut store,
        JournalAction::CompleteLocalUploadWithFetched {
            issue_id: ISSUE_ID.into(),
            journals: vec![journal_with_notes(10, "server notes"), journal(11)],
        },
    );

    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
    assert!(store.try_get_local_journal(ISSUE_ID).is_none());
}

#[test]
#[should_panic(expected = "local journal is not registered for issue 1")]
fn complete_local_upload_with_fetched_panics_when_the_local_entry_is_missing() {
    let mut store = store_with(&[]);

    apply(
        &mut store,
        JournalAction::CompleteLocalUploadWithFetched {
            issue_id: ISSUE_ID.into(),
            journals: vec![],
        },
    );
}

#[test]
#[should_panic(
    expected = "cannot complete local journal upload for issue 1 while it is local only"
)]
fn complete_local_upload_with_fetched_panics_when_the_local_entry_is_local_only() {
    let mut store = store_with(&[]);
    create_local(&mut store);

    apply(
        &mut store,
        JournalAction::CompleteLocalUploadWithFetched {
            issue_id: ISSUE_ID.into(),
            journals: vec![],
        },
    );
}

#[test]
fn complete_local_upload_without_fetch_removes_the_draft_and_keeps_remote_journals() {
    let mut store = edited_store(&[10], 10);
    create_local(&mut store);
    start_local(&mut store);

    apply(
        &mut store,
        JournalAction::CompleteLocalUploadWithoutFetch {
            issue_id: ISSUE_ID.into(),
        },
    );

    assert!(store.try_get_local_journal(ISSUE_ID).is_none());
    assert_eq!(journal_ids(&store), vec![10]);
    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
}

#[test]
#[should_panic(
    expected = "cannot complete local journal upload for issue 1 while it is local only"
)]
fn complete_local_upload_without_fetch_panics_when_the_local_entry_is_local_only() {
    let mut store = store_with(&[]);
    create_local(&mut store);

    apply(
        &mut store,
        JournalAction::CompleteLocalUploadWithoutFetch {
            issue_id: ISSUE_ID.into(),
        },
    );
}

#[test]
#[should_panic(expected = "local journal is not registered for issue 1")]
fn get_local_journal_panics_for_an_issue_without_a_local_journal() {
    let store = store_with(&[]);

    store.get_local_journal(ISSUE_ID);
}

/// 対象の状態ではActionを受理できないことを、panicの文言で確かめる。
macro_rules! remote_action_panics {
    ($($name:ident: $setup:expr, $action:expr => $expected:literal;)+) => {
        $(
            #[test]
            #[should_panic(expected = $expected)]
            fn $name() {
                let mut store: Store = $setup;

                apply(&mut store, $action);
            }
        )+
    };
}

remote_action_panics! {
    edit_remote_notes_panics_while_the_journal_is_uploading:
        uploading_store(&[10], 10),
        JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            notes: "late edit".to_string(),
        } => "cannot edit remote journal 10 while it is uploading";
    edit_remote_notes_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
            notes: "edit".to_string(),
        } => "remote journal 11 is not registered for issue 1";
    start_remote_upload_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
        } => "remote journal 11 is not registered for issue 1";
    fail_remote_upload_panics_when_the_journal_is_synced:
        store_with(&[10]),
        JournalAction::FailRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            message: "failure".to_string(),
        } => "cannot fail remote journal 10 upload while it is synced";
    fail_remote_upload_panics_when_the_journal_is_edited:
        edited_store(&[10], 10),
        JournalAction::FailRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            message: "failure".to_string(),
        } => "cannot fail remote journal 10 upload while it is edited";
    fail_remote_upload_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::FailRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
            message: "failure".to_string(),
        } => "remote journal 11 is not registered for issue 1";
    detect_remote_upload_conflict_panics_when_the_journal_is_synced:
        store_with(&[10]),
        JournalAction::DetectRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            server_notes: "server".to_string(),
        } => "cannot detect remote journal 10 upload conflict while it is synced";
    detect_remote_upload_conflict_panics_when_the_journal_is_edited:
        edited_store(&[10], 10),
        JournalAction::DetectRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            server_notes: "server".to_string(),
        } => "cannot detect remote journal 10 upload conflict while it is edited";
    detect_remote_upload_conflict_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::DetectRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
            server_notes: "server".to_string(),
        } => "remote journal 11 is not registered for issue 1";
    cancel_remote_upload_conflict_panics_when_the_journal_is_synced:
        store_with(&[10]),
        JournalAction::CancelRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        } => "cannot cancel remote journal 10 upload conflict while it is synced";
    cancel_remote_upload_conflict_panics_when_the_journal_is_edited:
        edited_store(&[10], 10),
        JournalAction::CancelRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        } => "cannot cancel remote journal 10 upload conflict while it is edited";
    cancel_remote_upload_conflict_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::CancelRemoteUploadConflict {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
        } => "remote journal 11 is not registered for issue 1";
    complete_remote_upload_panics_when_the_journal_is_synced:
        store_with(&[10]),
        JournalAction::CompleteRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            notes: "notes".to_string(),
        } => "cannot complete remote journal 10 upload while it is synced";
    complete_remote_upload_panics_when_the_journal_is_edited:
        edited_store(&[10], 10),
        JournalAction::CompleteRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            notes: "notes".to_string(),
        } => "cannot complete remote journal 10 upload while it is edited";
    complete_remote_upload_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::CompleteRemoteUpload {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
            notes: "notes".to_string(),
        } => "remote journal 11 is not registered for issue 1";
    remove_missing_remote_journal_panics_when_the_journal_is_synced:
        store_with(&[10]),
        JournalAction::RemoveMissingRemoteJournal {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        } => "cannot remove remote journal 10 while it is synced";
    remove_missing_remote_journal_panics_when_the_journal_is_edited:
        edited_store(&[10], 10),
        JournalAction::RemoveMissingRemoteJournal {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
        } => "cannot remove remote journal 10 while it is edited";
    remove_missing_remote_journal_panics_when_the_journal_is_not_registered:
        store_with(&[10]),
        JournalAction::RemoveMissingRemoteJournal {
            issue_id: ISSUE_ID.into(),
            journal_id: 11.into(),
        } => "remote journal 11 is not registered for issue 1";
}

fn child(id: u16, subject: &str) -> IssueChild {
    IssueChild {
        id: IssueId::new(id),
        tracker_id: 1.into(),
        subject: subject.to_string(),
        children: vec![],
    }
}

/// Journal 10を編集し、Issue属性も編集して保存を始めたStore。
fn uploading_issue_store() -> Store {
    let mut store = edited_store(&[10, 11], 10);
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "local body".to_string(),
        }
        .into(),
    );
    store.consume_action(
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        }
        .into(),
    );
    store
}

#[test]
fn upload_conflicts_take_in_fetched_journals_and_children_but_keep_the_issue_base() {
    let mut store = uploading_issue_store();
    let mut server_issue = issue_with_journals(
        ISSUE_ID,
        vec![journal_with_notes(10, "server notes"), journal(12)],
    );
    server_issue.issue.description = "server body".to_string();

    store.consume_action(
        IssueAction::UploadConflictsDetected {
            server_issue,
            conflicts: vec![],
            children: vec![child(2, "fetched child")],
        }
        .into(),
    );

    assert_eq!(journal_ids(&store), vec![10, 12]);
    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
    assert_eq!(
        store.get_issue_children(ISSUE_ID),
        &[child(2, "fetched child")]
    );
    let (issue, issue_state) = store.get_issue(ISSUE_ID);
    assert_eq!(issue_state, IssueState::Uploading);
    assert_eq!(issue.description(), "local body");
}

#[test]
fn upload_succeeded_replaces_the_issue_base_and_children_and_keeps_journal_edits() {
    let mut store = uploading_issue_store();
    let mut confirmed = issue_with_journals(ISSUE_ID, vec![journal(10), journal(11), journal(12)]);
    confirmed.issue.description = "local body".to_string();

    store.consume_action(
        IssueAction::UploadSucceeded {
            issue: confirmed,
            children: vec![child(2, "confirmed child")],
        }
        .into(),
    );

    let (issue, issue_state) = store.get_issue(ISSUE_ID);
    assert_eq!(issue_state, IssueState::Synced);
    assert_eq!(issue.description(), "local body");
    assert_eq!(journal_ids(&store), vec![10, 11, 12]);
    assert_eq!(
        state(&store, 10),
        edited_state("remote notes 10", "edited notes")
    );
    assert_eq!(
        store.get_issue_children(ISSUE_ID),
        &[child(2, "confirmed child")]
    );
}

#[test]
#[should_panic(expected = "cannot complete issue upload while issue 1 is Edited")]
fn upload_succeeded_panics_unless_the_issue_is_uploading() {
    let mut store = edited_store(&[10], 10);
    store.consume_action(
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "local body".to_string(),
        }
        .into(),
    );

    store.consume_action(
        IssueAction::UploadSucceeded {
            issue: issue_with_journals(ISSUE_ID, vec![journal(10)]),
            children: vec![],
        }
        .into(),
    );
}
