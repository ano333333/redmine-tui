use crate::stores::{Dispatcher, JournalAction, RemoteJournalUploadConflict};
use crate::vos::{IssueId, JournalId, JournalKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteJournalUploadRetry {
    pub id: JournalId,
    pub issue_id: IssueId,
    pub final_notes: String,
    pub displayed_server_notes: String,
}

/// 競合解決後のnotesをStoreへ反映し、再試行に必要なsnapshotを返す。
pub fn continue_remote_journal_upload(
    dispatcher: &mut Dispatcher,
    id: JournalId,
    final_notes: String,
) -> RemoteJournalUploadRetry {
    let conflict = dispatcher
        .store()
        .get_remote_journal_upload_conflict(id)
        .expect("Remote Journalのアップロード続行には競合情報が必要です");
    let (retry, actions) = continue_remote_journal_upload_actions(conflict, final_notes);
    for action in actions {
        dispatcher.dispatch(action);
    }
    retry
}

fn continue_remote_journal_upload_actions(
    conflict: &RemoteJournalUploadConflict,
    final_notes: String,
) -> (RemoteJournalUploadRetry, [JournalAction; 2]) {
    let retry = RemoteJournalUploadRetry {
        id: conflict.id,
        issue_id: conflict.issue_id,
        final_notes: final_notes.clone(),
        displayed_server_notes: conflict.server.notes.clone(),
    };
    let actions = [
        JournalAction::UpdateUploadingRemoteNotes {
            id: conflict.id,
            notes: final_notes,
        },
        JournalAction::ClearRemoteUploadConflict {
            id: conflict.id,
            issue_id: conflict.issue_id,
        },
    ];
    (retry, actions)
}

#[cfg(test)]
mod tests {
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{Dispatcher, JournalAction, RemoteJournalUploadConflict};
    use crate::vos::{IssueId, JournalId, JournalKey};

    use super::{continue_remote_journal_upload, continue_remote_journal_upload_actions};

    #[test]
    fn generates_update_then_clear_actions_and_exact_retry() {
        let dispatcher = conflicted_dispatcher();
        let conflict = dispatcher
            .store()
            .get_remote_journal_upload_conflict(1.into())
            .unwrap();

        let (retry, actions) =
            continue_remote_journal_upload_actions(conflict, "final notes".to_string());

        assert_eq!(retry.id, JournalId::new(1));
        assert_eq!(retry.issue_id, IssueId::new(3));
        assert_eq!(retry.final_notes, "final notes");
        assert_eq!(retry.displayed_server_notes, "server notes");
        let [
            JournalAction::UpdateUploadingRemoteNotes { id, notes },
            JournalAction::ClearRemoteUploadConflict {
                id: clear_id,
                issue_id,
            },
        ] = actions
        else {
            panic!("continue actions must update notes then clear the conflict")
        };
        assert_eq!((id, notes), (1.into(), "final notes".to_string()));
        assert_eq!((clear_id, issue_id), (1.into(), 3.into()));
    }

    #[test]
    #[should_panic(expected = "Remote Journalのアップロード続行には競合情報が必要です")]
    fn missing_conflict_is_an_internal_error() {
        continue_remote_journal_upload(
            &mut Dispatcher::new(),
            JournalId::new(1),
            "notes".to_string(),
        );
    }

    fn conflicted_dispatcher() -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        dispatcher.dispatch(crate::stores::IssueAction::Load { id: 3.into() });
        dispatcher.consume_action();
        dispatcher.dispatch(crate::stores::Action::LoadJournal { id: 1.into() });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditRemoteNotes {
            id: 1.into(),
            notes: "local notes".to_string(),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::StartUpload {
            key: JournalKey::Remote(1.into()),
        });
        dispatcher.consume_action();
        let mut server = parse_journal_yaml(1.into());
        server.notes = "server notes".to_string();
        dispatcher.dispatch(JournalAction::UploadConflictsDetected {
            conflict: RemoteJournalUploadConflict {
                id: 1.into(),
                issue_id: 3.into(),
                before: "before".to_string(),
                after: "local notes".to_string(),
                server,
            },
        });
        dispatcher.consume_action();
        dispatcher
    }
}
