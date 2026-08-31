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

impl JournalStore {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(super) fn entry(&self, key: JournalKey) -> Option<&JournalEntry> {
        self.entries.get(&key)
    }

    pub(super) fn load_fixture_remote(&mut self, journal: Journal, issue_id: IssueId) {
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
