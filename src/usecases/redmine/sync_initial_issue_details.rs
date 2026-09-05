use std::collections::HashSet;

use crate::entities::{IssueAggregate, Journal};
use crate::stores::{Action, IssueAction, JournalAction};
use crate::vos::JournalKey;

/// 初回取得したRemote Journalを先に、参照するIssueを最後に並べる。
pub fn initial_issue_details_actions(
    aggregate: IssueAggregate,
    journals: Vec<Journal>,
) -> Vec<Action> {
    // FIXME: Define queued-action composition, transition violations, and partial
    // application when Store-wide abnormal-state handling is designed.
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
    let mut actions = Vec::with_capacity(journals.len() + 1);
    for journal in journals {
        actions.push(JournalAction::RegisterRemote { journal, issue_id }.into());
    }
    actions.push(
        IssueAction::FetchSucceeded {
            id: issue_id,
            issue: aggregate,
        }
        .into(),
    );
    actions
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{Action, IssueAction, JournalAction};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueStatusId, JournalId, JournalKey, LocalJournalId};

    use super::initial_issue_details_actions;

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
    fn keeps_journal_input_order_and_places_the_issue_last() {
        let (aggregate, mut journals) = input(&[1, 2]);
        journals.reverse();

        let actions = initial_issue_details_actions(aggregate, journals);

        let [
            Action::Journal(JournalAction::RegisterRemote { journal: first, .. }),
            Action::Journal(JournalAction::RegisterRemote {
                journal: second, ..
            }),
            Action::Issue(IssueAction::FetchSucceeded { issue, .. }),
        ] = actions.as_slice()
        else {
            panic!("journals must precede the fetched issue")
        };
        assert_eq!(
            (first.id, second.id),
            (JournalId::new(2), JournalId::new(1))
        );
        assert_eq!(
            issue.journal_keys,
            vec![
                JournalKey::Remote(JournalId::new(1)),
                JournalKey::Remote(JournalId::new(2)),
            ]
        );
    }

    #[test]
    fn rejects_structurally_invalid_inputs() {
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
            let (mut aggregate, _) = input(&[]);
            aggregate.journal_keys = keys;
            let journals = journal_ids
                .into_iter()
                .map(|id| parse_journal_yaml(JournalId::new(id)))
                .collect();

            let result = catch_unwind(AssertUnwindSafe(|| {
                initial_issue_details_actions(aggregate, journals)
            }));

            assert!(result.is_err());
        }
    }
}
