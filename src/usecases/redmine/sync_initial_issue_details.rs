use std::collections::HashSet;

use crate::entities::{IssueAggregate, Journal};
use crate::stores::{Dispatcher, IssueAction, JournalAction};
use crate::vos::JournalKey;

/// 初回取得したRemote Journalを先に、参照するIssueを最後にdispatchする。
pub fn sync_initial_issue_details(
    dispatcher: &mut Dispatcher,
    aggregate: IssueAggregate,
    journals: Vec<Journal>,
) {
    if dispatcher.consume_actinos_len() != 0 {
        panic!("initial issue details sync requires an empty action queue");
    }
    let mut keys = HashSet::new();
    let remote_ids = aggregate
        .journal_keys
        .iter()
        .map(|key| match key {
            JournalKey::Remote(id) if keys.insert(*key) => *id,
            JournalKey::Remote(_) => panic!("journal keys must be unique"),
            JournalKey::Local(_) => panic!("initial sync cannot contain a local journal key"),
        })
        .collect::<HashSet<_>>();
    let mut journal_ids = HashSet::new();
    if journals
        .iter()
        .any(|journal| !journal_ids.insert(journal.id))
    {
        panic!("remote journal IDs must be unique");
    }
    if remote_ids != journal_ids {
        panic!("journal keys and journals must contain the same remote IDs");
    }

    let issue_id = aggregate.issue.id;
    if dispatcher.store().get_issue_state(issue_id).is_some() {
        panic!("cannot initially sync an issue that already has state");
    }
    if dispatcher.store().has_journal_entry_for_issue(issue_id) {
        panic!("cannot initially sync an issue that already has a journal");
    }
    if remote_ids.iter().any(|id| {
        dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(*id))
            .is_some()
    }) {
        panic!("cannot initially sync an existing remote journal");
    }
    for journal in journals {
        dispatcher.dispatch(JournalAction::RegisterRemote { journal, issue_id });
    }
    dispatcher.dispatch(IssueAction::Sync { issue: aggregate });
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{Dispatcher, JournalEntry, RemoteJournalState};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId, JournalKey, LocalJournalId};

    use super::sync_initial_issue_details;

    fn input(
        ids: &[u16],
    ) -> (
        crate::entities::IssueAggregate,
        Vec<crate::entities::Journal>,
    ) {
        let mut aggregate =
            sample_issue_aggregate(3, "synced", IssueStatusId::new(1), None, None, None, 0);
        aggregate.journal_keys = ids
            .iter()
            .map(|id| JournalKey::Remote(JournalId::new(*id)))
            .collect();
        let journals = ids
            .iter()
            .map(|id| parse_journal_yaml(JournalId::new(*id)))
            .collect();
        (aggregate, journals)
    }

    #[test]
    fn accepts_unordered_journals_but_keeps_journal_keys_as_the_display_order() {
        let mut dispatcher = Dispatcher::new();
        let (aggregate, mut journals) = input(&[1, 2]);
        journals.reverse();

        sync_initial_issue_details(&mut dispatcher, aggregate, journals);

        assert_eq!(dispatcher.consume_actinos_len(), 3);
        assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());

        dispatcher.consume_action();
        assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());
        assert!(matches!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(2))),
            Some(JournalEntry::Remote {
                issue_id,
                state: RemoteJournalState::Synced,
                notes_diff: None,
                ..
            }) if *issue_id == IssueId::new(3)
        ));
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
                .is_none()
        );

        // Temporary orphan entries are allowed while the queue is consumed.
        dispatcher.consume_action();
        assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
                .is_some()
        );

        dispatcher.consume_action();
        let (issue, _) = dispatcher
            .store()
            .get_issue(IssueId::new(3))
            .expect("the final action should install the issue");
        assert_eq!(
            issue.journal_keys,
            vec![
                JournalKey::Remote(JournalId::new(1)),
                JournalKey::Remote(JournalId::new(2)),
            ]
        );
    }

    #[test]
    fn structural_validation_failure_leaves_the_queue_and_store_unchanged() {
        for (keys, journal_ids) in [
            (
                vec![
                    JournalKey::Remote(JournalId::new(1)),
                    JournalKey::Remote(JournalId::new(1)),
                ],
                vec![1, 2],
            ),
            (vec![JournalKey::Local(LocalJournalId::new(1))], vec![1]),
            (vec![JournalKey::Remote(JournalId::new(1))], vec![1, 1]),
            (
                vec![
                    JournalKey::Remote(JournalId::new(1)),
                    JournalKey::Remote(JournalId::new(2)),
                ],
                vec![1],
            ),
            (vec![JournalKey::Remote(JournalId::new(1))], vec![1, 2]),
        ] {
            let mut dispatcher = Dispatcher::new();
            let (mut aggregate, _) = input(&[]);
            aggregate.journal_keys = keys;
            let journals = journal_ids
                .into_iter()
                .map(|id| parse_journal_yaml(JournalId::new(id)))
                .collect();

            let result = catch_unwind(AssertUnwindSafe(|| {
                sync_initial_issue_details(&mut dispatcher, aggregate, journals)
            }));

            assert!(result.is_err());
            assert_eq!(dispatcher.consume_actinos_len(), 0);
            assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());
            assert!(
                dispatcher
                    .store()
                    .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
                    .is_none()
            );
        }
    }

    #[test]
    fn initial_sync_rejects_an_existing_issue_before_dispatching_any_actions() {
        let mut dispatcher = Dispatcher::new();
        let (existing, _) = input(&[]);
        dispatcher.dispatch(crate::stores::IssueAction::Sync { issue: existing });
        dispatcher.consume_action();
        let (aggregate, journals) = input(&[1]);

        let result = catch_unwind(AssertUnwindSafe(|| {
            sync_initial_issue_details(&mut dispatcher, aggregate, journals)
        }));

        assert!(result.is_err());
        assert_eq!(dispatcher.consume_actinos_len(), 0);
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
                .is_none()
        );
    }

    #[test]
    fn initial_sync_rejects_an_existing_remote_id_before_dispatching_any_actions() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(crate::stores::JournalAction::RegisterRemote {
            journal: parse_journal_yaml(JournalId::new(1)),
            issue_id: IssueId::new(4),
        });
        dispatcher.consume_action();
        let (aggregate, journals) = input(&[1]);

        let result = catch_unwind(AssertUnwindSafe(|| {
            sync_initial_issue_details(&mut dispatcher, aggregate, journals)
        }));

        assert!(result.is_err());
        assert_eq!(dispatcher.consume_actinos_len(), 0);
        let Some(JournalEntry::Remote { issue_id, .. }) = dispatcher
            .store()
            .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
        else {
            panic!("existing journal should remain stored");
        };
        assert_eq!(*issue_id, IssueId::new(4));
        assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());
    }

    #[test]
    fn initial_sync_rejects_each_kind_of_pending_action_without_changing_the_queue() {
        for pending_journal in [false, true] {
            let mut dispatcher = Dispatcher::new();
            let local_id = dispatcher.new_local_journal_id();
            if pending_journal {
                dispatcher.dispatch(crate::stores::JournalAction::CreateLocal {
                    id: local_id,
                    issue_id: IssueId::new(9),
                    notes: "pending".to_string(),
                });
            } else {
                dispatcher.dispatch(crate::stores::IssueAction::Sync {
                    issue: sample_issue_aggregate(
                        9,
                        "pending",
                        IssueStatusId::new(1),
                        None,
                        None,
                        None,
                        0,
                    ),
                });
            }
            let (aggregate, journals) = input(&[1]);

            let result = catch_unwind(AssertUnwindSafe(|| {
                sync_initial_issue_details(&mut dispatcher, aggregate, journals)
            }));

            assert!(result.is_err());
            assert_eq!(dispatcher.consume_actinos_len(), 1);
            assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());
            assert!(
                dispatcher
                    .store()
                    .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
                    .is_none()
            );
            dispatcher.consume_action();
            if pending_journal {
                assert!(
                    dispatcher
                        .store()
                        .get_journal_entry(JournalKey::Local(local_id))
                        .is_some()
                );
            } else {
                assert!(dispatcher.store().get_issue(IssueId::new(9)).is_some());
            }
        }
    }

    #[test]
    fn initial_sync_rejects_an_existing_local_owner_before_dispatching_any_actions() {
        let mut dispatcher = Dispatcher::new();
        let local_id = dispatcher.new_local_journal_id();
        dispatcher.dispatch(crate::stores::JournalAction::CreateLocal {
            id: local_id,
            issue_id: IssueId::new(3),
            notes: "local draft".to_string(),
        });
        dispatcher.consume_action();
        let (aggregate, journals) = input(&[1]);

        let result = catch_unwind(AssertUnwindSafe(|| {
            sync_initial_issue_details(&mut dispatcher, aggregate, journals)
        }));

        assert!(result.is_err());
        assert_eq!(dispatcher.consume_actinos_len(), 0);
        assert!(dispatcher.store().get_issue(IssueId::new(3)).is_none());
        assert!(matches!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Local(local_id)),
            Some(JournalEntry::Local { journal, state: crate::stores::LocalJournalState::LocalOnly })
                if journal.issue_id == IssueId::new(3) && journal.notes == "local draft"
        ));
        assert!(
            dispatcher
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
                .is_none()
        );
    }
}
