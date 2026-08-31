use std::collections::HashMap;

use crate::entities::{Journal, LocalJournal};
use crate::vos::{IssueId, JournalKey, JournalNotesDiff};

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
}
