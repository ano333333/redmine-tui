use crate::stores::{Dispatcher, JournalAction, RemoteJournalUploadConflict};
use crate::vos::{JournalId, JournalKey};

/// Remote Journalの競合情報を破棄してアップロードを取り消すActionをdispatchする。
pub fn cancel_remote_journal_upload(dispatcher: &mut Dispatcher, id: JournalId) {
    let conflict = dispatcher
        .store()
        .get_remote_journal_upload_conflict(id)
        .expect("Remote Journalのアップロード取消には競合情報が必要です");
    for action in cancel_remote_journal_upload_actions(conflict) {
        dispatcher.dispatch(action);
    }
}

fn cancel_remote_journal_upload_actions(
    conflict: &RemoteJournalUploadConflict,
) -> [JournalAction; 2] {
    [
        JournalAction::ClearRemoteUploadConflict {
            id: conflict.id,
            issue_id: conflict.issue_id,
        },
        JournalAction::CancelUpload {
            key: JournalKey::Remote(conflict.id),
        },
    ]
}

#[cfg(test)]
mod tests {
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{Dispatcher, JournalAction, RemoteJournalUploadConflict};
    use crate::vos::{JournalId, JournalKey};

    use super::{cancel_remote_journal_upload, cancel_remote_journal_upload_actions};

    #[test]
    fn generates_clear_then_cancel_actions_with_exact_payloads() {
        let dispatcher = conflicted_dispatcher();
        let conflict = dispatcher
            .store()
            .get_remote_journal_upload_conflict(1.into())
            .unwrap();

        let actions = cancel_remote_journal_upload_actions(conflict);

        let [
            JournalAction::ClearRemoteUploadConflict { id, issue_id },
            JournalAction::CancelUpload { key },
        ] = actions
        else {
            panic!("cancel actions must clear the conflict then cancel the upload")
        };
        assert_eq!((id, issue_id), (1.into(), 3.into()));
        assert_eq!(key, JournalKey::Remote(1.into()));
    }

    #[test]
    #[should_panic(expected = "Remote Journalのアップロード取消には競合情報が必要です")]
    fn missing_conflict_is_an_internal_error() {
        cancel_remote_journal_upload(&mut crate::stores::Dispatcher::new(), JournalId::new(1));
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
