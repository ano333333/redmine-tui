use std::collections::HashMap;

use crate::entities::{Journal, LocalJournal};
use crate::vos::{IssueId, JournalId, JournalKey, JournalNotesDiff, LocalJournalId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteJournalState {
    Synced,
    Edited,
    Uploading,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalJournalState {
    LocalOnly,
    Uploading,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalUploadFailureStage {
    RemoteFetch,
    RemotePut,
    LocalPut,
    LocalRefresh,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalUploadFailure {
    pub stage: JournalUploadFailureStage,
    pub message: String,
}

#[derive(Clone)]
pub enum JournalEntry {
    Remote {
        journal: Journal,
        issue_id: IssueId,
        state: RemoteJournalState,
        notes_diff: Option<JournalNotesDiff>,
    },
    Local {
        journal: LocalJournal,
        state: LocalJournalState,
    },
}

#[derive(Clone)]
pub struct RemoteJournalUploadConflict {
    pub id: JournalId,
    pub issue_id: IssueId,
    pub before: String,
    pub after: String,
    pub server: Journal,
}

pub enum JournalAction {
    RegisterRemote {
        journal: Journal,
        issue_id: IssueId,
    },
    /// Applies one Remote Journal returned by a server fetch.
    ///
    /// A missing entry is registered as [`RemoteJournalState::Synced`]. An
    /// existing `Synced` entry owned by the same Issue is replaced with the
    /// server value. `Edited` and `Uploading` entries are left unchanged to
    /// preserve local edits. An entry owned by another Issue is rejected.
    SyncFetchedRemote {
        journal: Journal,
        issue_id: IssueId,
    },
    /// Removes a server-missing Remote Journal only when it is `Synced` and
    /// owned by the specified Issue.
    ///
    /// `Edited` and `Uploading` entries are rejected to preserve local edits.
    /// Entries owned by another Issue and missing Remote entries are also
    /// rejected. The action addresses only a Remote key derived from its
    /// `JournalId`, so a Local entry with the same numeric value is unaffected.
    RemoveSyncedRemote {
        id: JournalId,
        issue_id: IssueId,
    },
    /// Creates a Local Journal using an ID the caller reserved from the Dispatcher
    /// that will consume this action.
    CreateLocal {
        /// A ID newly returned by `Dispatcher::new_local_journal_id`.
        id: LocalJournalId,
        issue_id: IssueId,
        notes: String,
    },
    EditLocalNotes {
        id: LocalJournalId,
        notes: String,
    },
    EditRemoteNotes {
        id: JournalId,
        notes: String,
    },
    StartUpload {
        key: JournalKey,
    },
    /// Updates the retry value while preserving the original `diff.before`.
    UpdateUploadingRemoteNotes {
        id: JournalId,
        notes: String,
    },
    /// Removes the matching conflict snapshot without changing its Journal entry.
    ClearRemoteUploadConflict {
        id: JournalId,
        issue_id: IssueId,
    },
    /// Returns an uploading Remote Journal to `Edited` without changing its diff.
    CancelUpload {
        key: JournalKey,
    },
    FailUpload {
        key: JournalKey,
        failure: JournalUploadFailure,
    },
    CompleteRemoteUpload {
        id: JournalId,
        notes: String,
    },
    CompleteRemoteUploadFromFetch {
        journal: Journal,
        issue_id: IssueId,
    },
    /// Removes an `Uploading` Remote Journal
    RemoveUploadingRemote {
        id: JournalId,
        issue_id: IssueId,
    },
    /// Retains the before, after, and full server snapshot for conflict resolution.
    ///
    /// This action does not persist anything to the server. The target Remote
    /// entry, its notes diff, and its `Uploading` state remain unchanged.
    UploadConflictsDetected {
        conflict: RemoteJournalUploadConflict,
    },
}

pub(super) struct JournalStore {
    entries: HashMap<JournalKey, JournalEntry>,
    upload_conflicts: HashMap<JournalId, RemoteJournalUploadConflict>,
    upload_failures: HashMap<JournalKey, JournalUploadFailure>,
}

pub(crate) struct FetchedJournalsMerge {
    pub(crate) entries: HashMap<JournalKey, JournalEntry>,
    pub(crate) journal_keys: Vec<JournalKey>,
}

pub(crate) fn merge_fetched_journals(
    issue_id: IssueId,
    fetched: Vec<Journal>,
    old_keys: &[JournalKey],
    entries: &HashMap<JournalKey, JournalEntry>,
) -> FetchedJournalsMerge {
    let mut fetched_ids = std::collections::HashSet::new();
    for journal in &fetched {
        if !fetched_ids.insert(journal.id) {
            panic!("fetched journals contain duplicate IDs");
        }
        if matches!(
            entries.get(&JournalKey::Remote(journal.id)),
            Some(JournalEntry::Remote { issue_id: owner, .. }) if *owner != issue_id
        ) {
            panic!("remote journal is already owned by another issue");
        }
    }
    let mut old_key_set = std::collections::HashSet::new();
    let mut local_count = 0;
    for key in old_keys {
        if !old_key_set.insert(*key) {
            panic!("old journal keys contain duplicates");
        }
        if matches!(key, JournalKey::Local(_)) {
            local_count += 1;
        }
    }
    if local_count > 1 {
        panic!("old journal keys contain multiple local journals");
    }

    let mut entries = entries.clone();
    let mut journal_keys = Vec::with_capacity(fetched.len() + 1);

    for journal in fetched {
        let key = JournalKey::Remote(journal.id);
        let entry = match entries.remove(&key) {
            Some(
                entry @ JournalEntry::Remote {
                    issue_id: owner,
                    state: RemoteJournalState::Edited | RemoteJournalState::Uploading,
                    ..
                },
            ) if owner == issue_id => entry,
            _ => JournalEntry::Remote {
                journal,
                issue_id,
                state: RemoteJournalState::Synced,
                notes_diff: None,
            },
        };
        entries.insert(key, entry);
        journal_keys.push(key);
    }

    let mut local_keys = Vec::new();
    for key in old_keys.iter().copied() {
        if journal_keys.contains(&key) {
            continue;
        }
        match entries.get(&key) {
            Some(JournalEntry::Remote {
                issue_id: owner,
                state: RemoteJournalState::Edited | RemoteJournalState::Uploading,
                ..
            }) if *owner == issue_id => journal_keys.push(key),
            Some(JournalEntry::Local { journal, .. }) if journal.issue_id == issue_id => {
                local_keys.push(key)
            }
            Some(JournalEntry::Remote {
                issue_id: owner,
                state: RemoteJournalState::Synced,
                ..
            }) if *owner == issue_id => {
                entries.remove(&key);
            }
            _ => {}
        }
    }
    journal_keys.extend(local_keys);

    FetchedJournalsMerge {
        entries,
        journal_keys,
    }
}

impl JournalStore {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            upload_conflicts: HashMap::new(),
            upload_failures: HashMap::new(),
        }
    }

    pub(super) fn entry(&self, key: JournalKey) -> Option<&JournalEntry> {
        self.entries.get(&key)
    }

    pub(super) fn has_entry_for_issue(&self, issue_id: IssueId) -> bool {
        self.entries.values().any(|entry| match entry {
            JournalEntry::Remote { issue_id: id, .. } => *id == issue_id,
            JournalEntry::Local { journal, .. } => journal.issue_id == issue_id,
        })
    }

    pub(super) fn has_uploading_journal(&self, issue_id: IssueId) -> bool {
        self.entries.values().any(|entry| match entry {
            JournalEntry::Remote {
                issue_id: id,
                state,
                ..
            } => *id == issue_id && *state == RemoteJournalState::Uploading,
            JournalEntry::Local { journal, state } => {
                journal.issue_id == issue_id && *state == LocalJournalState::Uploading
            }
        })
    }

    pub(super) fn load_remote_fixture(&mut self, journal: Journal, issue_id: IssueId) {
        self.entries
            .entry(JournalKey::Remote(journal.id))
            .or_insert(JournalEntry::Remote {
                journal,
                issue_id,
                state: RemoteJournalState::Synced,
                notes_diff: None,
            });
    }

    pub(super) fn consume_action(&mut self, action: JournalAction) {
        match action {
            JournalAction::RegisterRemote { journal, issue_id } => {
                let key = JournalKey::Remote(journal.id);
                match self.entries.get(&key) {
                    Some(JournalEntry::Remote {
                        issue_id: owner, ..
                    }) if *owner != issue_id => {
                        panic!("remote journal is already owned by another issue")
                    }
                    Some(_) => panic!("remote journal already exists"),
                    None => {
                        self.entries.insert(
                            key,
                            JournalEntry::Remote {
                                journal,
                                issue_id,
                                state: RemoteJournalState::Synced,
                                notes_diff: None,
                            },
                        );
                    }
                }
            }
            JournalAction::SyncFetchedRemote { journal, issue_id } => {
                let key = JournalKey::Remote(journal.id);
                match self.entries.get_mut(&key) {
                    Some(JournalEntry::Remote {
                        journal: stored,
                        issue_id: owner,
                        state: RemoteJournalState::Synced,
                        ..
                    }) if *owner == issue_id => *stored = journal,
                    Some(JournalEntry::Remote {
                        issue_id: owner,
                        state: RemoteJournalState::Edited | RemoteJournalState::Uploading,
                        ..
                    }) if *owner == issue_id => {}
                    Some(_) => panic!("remote journal is already owned by another issue"),
                    None => {
                        self.entries.insert(
                            key,
                            JournalEntry::Remote {
                                journal,
                                issue_id,
                                state: RemoteJournalState::Synced,
                                notes_diff: None,
                            },
                        );
                    }
                }
            }
            JournalAction::RemoveSyncedRemote { id, issue_id } => {
                let key = JournalKey::Remote(id);
                match self.entries.get(&key) {
                    Some(JournalEntry::Remote {
                        issue_id: owner,
                        state: RemoteJournalState::Synced,
                        ..
                    }) if *owner == issue_id => {
                        self.entries.remove(&key);
                    }
                    Some(JournalEntry::Remote {
                        issue_id: owner, ..
                    }) if *owner != issue_id => panic!("remote journal belongs to another issue"),
                    Some(JournalEntry::Remote { .. }) => {
                        panic!("cannot remove a dirty remote journal")
                    }
                    _ => panic!("remote journal does not exist"),
                }
            }
            JournalAction::CreateLocal {
                id,
                issue_id,
                notes,
            } => {
                if self.entries.values().any(|entry| {
                    matches!(
                        entry,
                        JournalEntry::Local { journal, .. } if journal.issue_id == issue_id
                    )
                }) {
                    panic!("issue already has a local journal");
                }
                self.entries.insert(
                    JournalKey::Local(id),
                    JournalEntry::Local {
                        journal: LocalJournal {
                            id,
                            issue_id,
                            notes,
                        },
                        state: LocalJournalState::LocalOnly,
                    },
                );
            }
            JournalAction::EditLocalNotes { id, notes } => {
                let Some(JournalEntry::Local { journal, state }) =
                    self.entries.get_mut(&JournalKey::Local(id))
                else {
                    panic!("local journal does not exist");
                };
                if state == &LocalJournalState::Uploading {
                    panic!("cannot edit a local journal while uploading");
                }
                journal.notes = notes;
            }
            JournalAction::EditRemoteNotes { id, notes } => {
                let Some(JournalEntry::Remote {
                    journal,
                    state,
                    notes_diff,
                    ..
                }) = self.entries.get_mut(&JournalKey::Remote(id))
                else {
                    panic!("remote journal does not exist");
                };
                if state == &RemoteJournalState::Uploading {
                    panic!("cannot edit a remote journal while uploading");
                }
                let before = notes_diff
                    .as_ref()
                    .map_or_else(|| journal.notes.clone(), |diff| diff.before.clone());
                journal.notes = notes.clone();
                if notes == before {
                    *state = RemoteJournalState::Synced;
                    *notes_diff = None;
                } else {
                    *state = RemoteJournalState::Edited;
                    *notes_diff = Some(JournalNotesDiff {
                        before,
                        after: notes,
                    });
                }
            }
            JournalAction::StartUpload { key } => {
                let Some(entry) = self.entries.get_mut(&key) else {
                    panic!("journal does not exist");
                };
                match entry {
                    JournalEntry::Remote { state, .. } => match state {
                        RemoteJournalState::Edited => *state = RemoteJournalState::Uploading,
                        RemoteJournalState::Synced | RemoteJournalState::Uploading => {}
                    },
                    JournalEntry::Local { state, .. } => match state {
                        LocalJournalState::LocalOnly => *state = LocalJournalState::Uploading,
                        LocalJournalState::Uploading => {}
                    },
                }
                self.upload_failures.remove(&key);
            }
            JournalAction::UpdateUploadingRemoteNotes { id, notes } => {
                let Some(JournalEntry::Remote {
                    journal,
                    state,
                    notes_diff: Some(diff),
                    ..
                }) = self.entries.get_mut(&JournalKey::Remote(id))
                else {
                    panic!("uploading remote journal does not exist");
                };
                if state != &RemoteJournalState::Uploading {
                    panic!("remote journal is not uploading");
                }
                journal.notes = notes.clone();
                diff.after = notes;
            }
            JournalAction::ClearRemoteUploadConflict { id, issue_id } => {
                match self.entries.get(&JournalKey::Remote(id)) {
                    Some(JournalEntry::Remote {
                        issue_id: owner,
                        state: RemoteJournalState::Uploading,
                        ..
                    }) if *owner == issue_id => {}
                    Some(JournalEntry::Remote {
                        issue_id: owner, ..
                    }) if *owner != issue_id => {
                        panic!("remote journal belongs to another issue")
                    }
                    Some(JournalEntry::Remote { .. }) => {
                        panic!("remote journal is not uploading")
                    }
                    _ => panic!("remote journal does not exist"),
                }
                match self.upload_conflicts.get(&id) {
                    Some(conflict) if conflict.issue_id == issue_id => {}
                    Some(_) => panic!("remote journal conflict belongs to another issue"),
                    None => panic!("remote journal upload conflict does not exist"),
                }
                self.upload_conflicts.remove(&id);
            }
            JournalAction::CancelUpload { key } => {
                let Some(JournalEntry::Remote { state, .. }) = self.entries.get_mut(&key) else {
                    panic!("remote journal does not exist");
                };
                if state != &RemoteJournalState::Uploading {
                    panic!("remote journal is not uploading");
                }
                *state = RemoteJournalState::Edited;
            }
            JournalAction::FailUpload { key, failure } => {
                let Some(entry) = self.entries.get_mut(&key) else {
                    panic!("journal does not exist");
                };
                match entry {
                    JournalEntry::Remote { state, .. }
                        if state == &RemoteJournalState::Uploading =>
                    {
                        *state = RemoteJournalState::Edited;
                    }
                    JournalEntry::Local { state, .. } if state == &LocalJournalState::Uploading => {
                        *state = LocalJournalState::LocalOnly;
                    }
                    _ => panic!("journal is not uploading"),
                }
                self.upload_failures.insert(key, failure);
            }
            JournalAction::CompleteRemoteUpload { id, notes } => {
                let Some(JournalEntry::Remote {
                    journal,
                    state,
                    notes_diff,
                    ..
                }) = self.entries.get_mut(&JournalKey::Remote(id))
                else {
                    panic!("remote journal does not exist");
                };
                if state != &RemoteJournalState::Uploading {
                    panic!("remote journal is not uploading");
                }
                journal.notes = notes;
                *state = RemoteJournalState::Synced;
                *notes_diff = None;
                self.upload_failures.remove(&JournalKey::Remote(id));
            }
            JournalAction::CompleteRemoteUploadFromFetch { journal, issue_id } => {
                let key = JournalKey::Remote(journal.id);
                let Some(JournalEntry::Remote {
                    journal: stored,
                    issue_id: owner,
                    state,
                    notes_diff,
                }) = self.entries.get_mut(&key)
                else {
                    panic!("remote journal does not exist");
                };
                if *owner != issue_id {
                    panic!("remote journal belongs to another issue");
                }
                if state != &RemoteJournalState::Uploading {
                    panic!("remote journal is not uploading");
                }
                *stored = journal;
                *state = RemoteJournalState::Synced;
                *notes_diff = None;
                self.upload_failures.remove(&key);
            }
            JournalAction::RemoveUploadingRemote { id, issue_id } => {
                let key = JournalKey::Remote(id);
                let Some(JournalEntry::Remote {
                    issue_id: owner,
                    state,
                    ..
                }) = self.entries.get(&key)
                else {
                    panic!("remote journal does not exist");
                };
                if *owner != issue_id {
                    panic!("remote journal belongs to another issue");
                }
                if state != &RemoteJournalState::Uploading {
                    panic!("remote journal is not uploading");
                }
                self.entries.remove(&key);
                self.upload_failures.remove(&key);
            }
            JournalAction::UploadConflictsDetected { conflict } => {
                if conflict.server.id != conflict.id {
                    panic!("remote journal conflict server ID does not match");
                }
                match self.entries.get(&JournalKey::Remote(conflict.id)) {
                    Some(JournalEntry::Remote {
                        issue_id,
                        state: RemoteJournalState::Uploading,
                        ..
                    }) if *issue_id == conflict.issue_id => {}
                    Some(JournalEntry::Remote { issue_id, .. })
                        if *issue_id != conflict.issue_id =>
                    {
                        panic!("remote journal belongs to another issue")
                    }
                    Some(JournalEntry::Remote { .. }) => {
                        panic!("remote journal is not uploading")
                    }
                    _ => panic!("remote journal does not exist"),
                }
                self.upload_conflicts.insert(conflict.id, conflict);
            }
        }
    }

    pub(super) fn upload_conflict(&self, id: JournalId) -> Option<&RemoteJournalUploadConflict> {
        self.upload_conflicts.get(&id)
    }

    pub(super) fn upload_failure(&self, key: JournalKey) -> Option<&JournalUploadFailure> {
        self.upload_failures.get(&key)
    }
}
