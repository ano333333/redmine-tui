use crate::vos::JournalNotesDiff;

#[derive(Clone, Debug, PartialEq, Eq)]
enum RemoteJournalNotesDecision {
    AlreadyApplied,
    Upload {
        notes: String,
    },
    Conflict {
        before: String,
        after: String,
        server: String,
    },
}

fn compare_remote_journal_notes(
    diff: &JournalNotesDiff,
    server: &str,
) -> RemoteJournalNotesDecision {
    if server == diff.before {
        RemoteJournalNotesDecision::Upload {
            notes: diff.after.clone(),
        }
    } else if server == diff.after {
        RemoteJournalNotesDecision::AlreadyApplied
    } else {
        RemoteJournalNotesDecision::Conflict {
            before: diff.before.clone(),
            after: diff.after.clone(),
            server: server.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::vos::JournalNotesDiff;

    use super::{RemoteJournalNotesDecision, compare_remote_journal_notes};

    #[test]
    fn uploads_local_notes_when_the_server_is_unchanged() {
        let diff = JournalNotesDiff {
            before: "before".to_string(),
            after: "after".to_string(),
        };

        assert_eq!(
            compare_remote_journal_notes(&diff, "before"),
            RemoteJournalNotesDecision::Upload {
                notes: "after".to_string()
            }
        );
    }

    #[test]
    fn skips_upload_when_the_server_already_has_local_notes() {
        let diff = JournalNotesDiff {
            before: "before".to_string(),
            after: "after".to_string(),
        };

        assert_eq!(
            compare_remote_journal_notes(&diff, "after"),
            RemoteJournalNotesDecision::AlreadyApplied
        );
    }

    #[test]
    fn reports_all_three_values_when_both_sides_changed() {
        let diff = JournalNotesDiff {
            before: "before".to_string(),
            after: "local".to_string(),
        };

        assert_eq!(
            compare_remote_journal_notes(&diff, "server"),
            RemoteJournalNotesDecision::Conflict {
                before: "before".to_string(),
                after: "local".to_string(),
                server: "server".to_string(),
            }
        );
    }

    #[test]
    fn treats_empty_notes_as_regular_values() {
        let cleared = JournalNotesDiff {
            before: "before".to_string(),
            after: String::new(),
        };
        assert_eq!(
            compare_remote_journal_notes(&cleared, "before"),
            RemoteJournalNotesDecision::Upload {
                notes: String::new()
            }
        );

        let added = JournalNotesDiff {
            before: String::new(),
            after: "after".to_string(),
        };
        assert_eq!(
            compare_remote_journal_notes(&added, "after"),
            RemoteJournalNotesDecision::AlreadyApplied
        );
    }
}
