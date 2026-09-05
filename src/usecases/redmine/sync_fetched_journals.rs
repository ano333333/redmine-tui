use std::collections::HashMap;

use crate::entities::Journal;
use crate::stores::{Dispatcher, IssueAction, JournalAction, JournalEntry, merge_fetched_journals};
use crate::vos::{IssueId, JournalKey};

pub fn sync_fetched_journals(
    dispatcher: &mut Dispatcher,
    issue_id: IssueId,
    fetched: Vec<Journal>,
) {
    // FIXME: Define how multi-action usecases compose with already queued actions
    // when Store-wide abnormal-state handling is designed.
    let old_keys = dispatcher
        .store()
        .get_issue(issue_id)
        .expect("cannot sync journals for a missing issue")
        .0
        .journal_keys
        .clone();
    let mut entries = HashMap::new();
    for key in &old_keys {
        let entry = dispatcher
            .store()
            .get_journal_entry(*key)
            .expect("issue journal key must resolve before journal sync");
        match entry {
            JournalEntry::Remote {
                issue_id: owner, ..
            } if *owner != issue_id => panic!("journal belongs to another issue"),
            JournalEntry::Local { journal, .. } if journal.issue_id != issue_id => {
                panic!("journal belongs to another issue")
            }
            _ => {}
        }
        entries.insert(*key, entry.clone());
    }
    for journal in &fetched {
        let key = JournalKey::Remote(journal.id);
        if let Some(entry) = dispatcher.store().get_journal_entry(key) {
            entries.insert(key, entry.clone());
        }
    }

    let merged = merge_fetched_journals(issue_id, fetched.clone(), &old_keys, &entries);
    let obsolete = old_keys.iter().filter_map(|key| match key {
        JournalKey::Remote(id) if !merged.entries.contains_key(key) => Some(*id),
        _ => None,
    });
    for journal in fetched {
        dispatcher.dispatch(JournalAction::SyncFetchedRemote { journal, issue_id });
    }
    dispatcher.dispatch(IssueAction::ReplaceJournalKeys {
        id: issue_id,
        journal_keys: merged.journal_keys,
    });
    for id in obsolete {
        dispatcher.dispatch(JournalAction::RemoveSyncedRemote { id, issue_id });
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{
        Dispatcher, IssueAction, IssueState, JournalAction, JournalEntry, LocalJournalState,
        RemoteJournalState,
    };
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId, JournalKey};

    use super::sync_fetched_journals;

    fn populated_dispatcher() -> (Dispatcher, crate::vos::LocalJournalId) {
        let mut dispatcher = Dispatcher::new();
        let local_id = dispatcher.new_local_journal_id();
        for id in [1, 2, 4] {
            let mut journal = parse_journal_yaml(JournalId::new(id.min(3)));
            journal.id = JournalId::new(id);
            dispatcher.dispatch(JournalAction::RegisterRemote {
                journal,
                issue_id: IssueId::new(3),
            });
            dispatcher.consume_action();
        }
        dispatcher.dispatch(JournalAction::CreateLocal {
            id: local_id,
            issue_id: IssueId::new(3),
            notes: "local".to_string(),
        });
        dispatcher.consume_action();
        let mut issue =
            sample_issue_aggregate(3, "issue", IssueStatusId::new(1), None, None, None, 0);
        issue.journal_keys = vec![
            JournalKey::Remote(JournalId::new(1)),
            JournalKey::Remote(JournalId::new(2)),
            JournalKey::Remote(JournalId::new(4)),
            JournalKey::Local(local_id),
        ];
        dispatcher.dispatch(IssueAction::Sync { issue });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditRemoteNotes {
            id: JournalId::new(1),
            notes: "edited".to_string(),
        });
        dispatcher.consume_action();
        (dispatcher, local_id)
    }

    #[test]
    fn queues_upserts_then_keys_then_removals_without_dangling_references() {
        let (mut dispatcher, local_id) = populated_dispatcher();
        let mut fetched_two = parse_journal_yaml(JournalId::new(2));
        fetched_two.notes = "fresh".to_string();

        sync_fetched_journals(
            &mut dispatcher,
            IssueId::new(3),
            vec![fetched_two, parse_journal_yaml(JournalId::new(3))],
        );

        assert_eq!(dispatcher.consume_actinos_len(), 4);
        dispatcher.consume_action();
        dispatcher.consume_action();
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(3)))
                .is_some()
        );
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(4)))
                .is_some()
        );
        dispatcher.consume_action();
        let (issue, state) = dispatcher.store().get_issue(IssueId::new(3)).unwrap();
        assert_eq!(
            issue.journal_keys,
            vec![
                JournalKey::Remote(JournalId::new(2)),
                JournalKey::Remote(JournalId::new(3)),
                JournalKey::Remote(JournalId::new(1)),
                JournalKey::Local(local_id),
            ]
        );
        assert_eq!(state, &IssueState::Synced);
        dispatcher.consume_action();
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(4)))
                .is_none()
        );
        assert!(matches!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(1))),
            Some(JournalEntry::Remote {
                state: RemoteJournalState::Edited,
                ..
            })
        ));
        assert!(matches!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Local(local_id)),
            Some(JournalEntry::Local {
                state: LocalJournalState::LocalOnly,
                ..
            })
        ));
    }

    #[test]
    fn rejects_missing_issue_and_invalid_ownership_before_dispatching() {
        let mut missing = Dispatcher::new();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                sync_fetched_journals(&mut missing, IssueId::new(3), vec![])
            }))
            .is_err()
        );
        assert_eq!(missing.consume_actinos_len(), 0);

        let (mut collision, _) = populated_dispatcher();
        collision.dispatch(JournalAction::RegisterRemote {
            journal: parse_journal_yaml(JournalId::new(3)),
            issue_id: IssueId::new(4),
        });
        collision.consume_action();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                sync_fetched_journals(
                    &mut collision,
                    IssueId::new(3),
                    vec![parse_journal_yaml(JournalId::new(3))],
                )
            }))
            .is_err()
        );
        assert_eq!(collision.consume_actinos_len(), 0);
        assert!(matches!(
            collision
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(3))),
            Some(JournalEntry::Remote { issue_id, .. }) if *issue_id == IssueId::new(4)
        ));
    }

    #[test]
    fn rejects_invalid_existing_references_and_duplicate_fetch_before_dispatching() {
        let mut dangling = Dispatcher::new();
        let mut issue =
            sample_issue_aggregate(3, "issue", IssueStatusId::new(1), None, None, None, 0);
        issue.journal_keys = vec![JournalKey::Remote(JournalId::new(1))];
        dangling.dispatch(IssueAction::Sync { issue });
        dangling.consume_action();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                sync_fetched_journals(&mut dangling, IssueId::new(3), vec![])
            }))
            .is_err()
        );
        assert_eq!(dangling.consume_actinos_len(), 0);

        let (mut duplicate, _) = populated_dispatcher();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                sync_fetched_journals(
                    &mut duplicate,
                    IssueId::new(3),
                    vec![
                        parse_journal_yaml(JournalId::new(2)),
                        parse_journal_yaml(JournalId::new(2)),
                    ],
                )
            }))
            .is_err()
        );
        assert_eq!(duplicate.consume_actinos_len(), 0);
        assert_eq!(
            duplicate
                .store()
                .get_issue(IssueId::new(3))
                .unwrap()
                .0
                .journal_keys
                .len(),
            4
        );
    }
}
