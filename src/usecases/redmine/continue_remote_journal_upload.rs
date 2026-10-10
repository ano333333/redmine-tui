use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::clients::redmine::RedmineClient;
use crate::stores::{Dispatcher, RemoteJournalState};
use crate::usecases::UsecaseTask;
use crate::vos::{IssueId, JournalId};

use super::start_remote_journal_upload::upload_remote_journal_action;

/// 競合解決後のRemote Journal uploadを継続する。
///
/// # Panics
///
/// 対象Remote Journalが `Uploading { conflict: Some(_) }` でない場合にpanicする。
pub fn continue_remote_journal_upload<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    client: Arc<C>,
    issue_id: IssueId,
    journal_id: JournalId,
    resolved_notes: String,
) -> Option<UsecaseTask>
where
    C: RedmineClient + Send + Sync + 'static,
{
    let conflict_server_notes = {
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        let entry = store.get_remote_journal(issue_id, journal_id);
        match &entry.state {
            RemoteJournalState::Uploading {
                conflict: Some(conflict),
                ..
            } => conflict.server_notes.clone(),
            _ => panic!(
                "cannot continue remote journal upload unless remote journal {journal_id} is in conflict"
            ),
        }
    };

    // 前回の競合検出時点からのサーバー更新も検出するため、その時点の値を比較の基準にする。
    Some(Box::pin(async move {
        upload_remote_journal_action(
            client.as_ref(),
            issue_id,
            journal_id,
            &conflict_server_notes,
            &resolved_notes,
        )
        .await
        .into()
    }))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use crate::stores::{
        Action, DeletedJournalEntry, DeletedJournalState, Dispatcher, JournalAction,
        RemoteJournalState, RemoteJournalUploadConflict,
    };
    use crate::test_support::complete_usecase;
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::{IssueId, IssueStatusId, JournalId};

    use super::super::start_remote_journal_upload::tests::{
        StubClient, fetched, journal_with_notes, offline,
    };
    use super::continue_remote_journal_upload;

    const ISSUE_ID: IssueId = IssueId::new(1);
    const JOURNAL_ID: JournalId = JournalId::new(10);

    const CONFLICT_SERVER: &str = "conflicting notes";
    const RESOLVED: &str = "resolved notes";

    fn synced_dispatcher() -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        let mut issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        issue.journals = vec![journal_with_notes("remote notes")];
        crate::test_support::dispatch_loaded_issue(&mut dispatcher, issue);
        dispatcher
    }

    fn edited_dispatcher() -> Dispatcher {
        let mut dispatcher = synced_dispatcher();
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
            notes: "edited notes".to_string(),
        }));
        dispatcher.consume_action();
        dispatcher
    }

    fn uploading_dispatcher() -> Dispatcher {
        let mut dispatcher = edited_dispatcher();
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: ISSUE_ID,
            journal_id: JOURNAL_ID,
        }));
        dispatcher.consume_action();
        dispatcher
    }

    /// サーバーのnotesが`CONFLICT_SERVER`に変わっていたため、保存が競合した状態。
    fn conflict_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let mut dispatcher = uploading_dispatcher();
        dispatcher.dispatch(Action::Journal(JournalAction::DetectRemoteUploadConflict {
            journal_id: JOURNAL_ID,
            issue: fetched(vec![journal_with_notes(CONFLICT_SERVER)], "fetched").aggregate,
            children: vec![],
        }));
        dispatcher.consume_action();
        Rc::new(RefCell::new(dispatcher))
    }

    fn assert_panics(dispatcher: Dispatcher) {
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            continue_remote_journal_upload(
                dispatcher.clone(),
                Arc::new(StubClient::new(vec![], Ok(()))),
                ISSUE_ID,
                JOURNAL_ID,
                RESOLVED.to_string(),
            );
        }));
        assert!(result.is_err());
    }

    async fn continue_with(
        dispatcher: &Rc<RefCell<Dispatcher>>,
        client: &Arc<StubClient>,
    ) -> Vec<Action> {
        complete_usecase(continue_remote_journal_upload(
            dispatcher.clone(),
            client.clone(),
            ISSUE_ID,
            JOURNAL_ID,
            RESOLVED.to_string(),
        ))
        .await
    }

    fn dispatch_all(dispatcher: &Rc<RefCell<Dispatcher>>, actions: Vec<Action>) {
        for action in actions {
            dispatcher.borrow_mut().dispatch(action);
        }
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
    }

    fn state(dispatcher: &Rc<RefCell<Dispatcher>>) -> RemoteJournalState {
        dispatcher
            .borrow()
            .store()
            .get_remote_journal(ISSUE_ID, JOURNAL_ID)
            .state
            .clone()
    }

    #[test]
    fn panics_when_the_journal_is_synced() {
        assert_panics(synced_dispatcher());
    }

    #[test]
    fn panics_when_the_journal_is_edited() {
        assert_panics(edited_dispatcher());
    }

    #[test]
    fn panics_when_uploading_without_conflict() {
        assert_panics(uploading_dispatcher());
    }

    #[tokio::test]
    async fn server_still_at_the_conflict_notes_puts_the_resolved_notes_and_completes() {
        let dispatcher = conflict_dispatcher();
        let client = Arc::new(StubClient::new(
            vec![
                Ok(fetched(
                    vec![journal_with_notes(CONFLICT_SERVER)],
                    "fetched",
                )),
                Ok(fetched(vec![journal_with_notes(RESOLVED)], "fetched")),
            ],
            Ok(()),
        ));

        let actions = continue_with(&dispatcher, &client).await;

        assert_eq!(*client.put_notes.lock().unwrap(), vec![RESOLVED]);
        dispatch_all(&dispatcher, actions);
        assert_eq!(state(&dispatcher), RemoteJournalState::Synced);
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_remote_journal(ISSUE_ID, JOURNAL_ID)
                .journal
                .notes,
            RESOLVED
        );
    }

    #[tokio::test]
    async fn server_equal_to_the_resolved_notes_completes_without_a_put() {
        let dispatcher = conflict_dispatcher();
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched(vec![journal_with_notes(RESOLVED)], "fetched"))],
            Ok(()),
        ));

        let actions = continue_with(&dispatcher, &client).await;

        assert!(client.put_notes.lock().unwrap().is_empty());
        dispatch_all(&dispatcher, actions);
        assert_eq!(state(&dispatcher), RemoteJournalState::Synced);
    }

    #[tokio::test]
    async fn a_server_changed_again_replaces_the_conflict_without_a_put() {
        let dispatcher = conflict_dispatcher();
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched(
                vec![journal_with_notes("newer notes")],
                "fetched",
            ))],
            Ok(()),
        ));

        let actions = continue_with(&dispatcher, &client).await;

        assert!(client.put_notes.lock().unwrap().is_empty());
        dispatch_all(&dispatcher, actions);
        let RemoteJournalState::Uploading { conflict, .. } = state(&dispatcher) else {
            panic!("expected uploading state");
        };
        assert_eq!(
            conflict,
            Some(RemoteJournalUploadConflict {
                server_notes: "newer notes".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn a_journal_missing_from_the_get_result_evacuates_the_resolved_notes() {
        let dispatcher = conflict_dispatcher();
        let client = Arc::new(StubClient::new(
            vec![Ok(fetched(vec![], "fetched"))],
            Ok(()),
        ));

        let actions = continue_with(&dispatcher, &client).await;

        assert!(client.put_notes.lock().unwrap().is_empty());
        dispatch_all(&dispatcher, actions);
        assert_eq!(
            dispatcher.borrow().store().get_deleted_journals(ISSUE_ID),
            &[DeletedJournalEntry {
                original_id: JOURNAL_ID,
                notes: RESOLVED.to_string(),
                state: DeletedJournalState::Pending { failure: None },
            }]
        );
    }

    #[tokio::test]
    async fn get_failure_returns_to_edited_with_the_original_diff() {
        let dispatcher = conflict_dispatcher();
        let client = Arc::new(StubClient::new(vec![Err(offline())], Ok(())));

        let actions = continue_with(&dispatcher, &client).await;

        dispatch_all(&dispatcher, actions);
        let RemoteJournalState::Edited { diff, failure } = state(&dispatcher) else {
            panic!("expected edited state");
        };
        assert_eq!(diff.before, "remote notes");
        assert_eq!(diff.after, "edited notes");
        assert!(failure.is_some());
    }
}
