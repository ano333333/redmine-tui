use std::collections::HashMap;

use crate::entities::{Journal, LocalJournal};
use crate::vos::{IssueId, JournalKey, JournalNotesDiff, LocalJournalId};

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
                let Some(JournalEntry::Local { journal, .. }) =
                    self.entries.get_mut(&JournalKey::Local(id))
                else {
                    panic!("local journal does not exist");
                };
                journal.notes = notes;
            }
        }
    }
}
