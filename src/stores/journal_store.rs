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

pub enum JournalAction {
    RegisterRemote {
        journal: Journal,
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
    FailUpload {
        key: JournalKey,
    },
    CompleteRemoteUpload {
        id: JournalId,
        notes: String,
    },
}

pub(super) struct JournalStore {
    entries: HashMap<JournalKey, JournalEntry>,
}

pub(super) struct FetchedJournalsMerge {
    pub(super) entries: HashMap<JournalKey, JournalEntry>,
    pub(super) journal_keys: Vec<JournalKey>,
}

pub(super) fn merge_fetched_journals(
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
            }
            JournalAction::FailUpload { key } => {
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
            }
        }
    }
}
