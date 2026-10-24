use super::{
    Action, AttachmentAction, AttachmentUpload, AttachmentUploadStage, IssueAction, IssueState,
    JournalAction, RemoteJournalState, Store,
};
use crate::entities::{Attachment, IssueAggregate, Journal};
use crate::test_support::{local_datetime, sample_issue_aggregate};
use crate::vos::{AttachmentId, IssueId, JournalId};

const ISSUE_ID: u16 = 1;

fn journal(journal_id: u16) -> Journal {
    Journal {
        id: JournalId::new(journal_id),
        issue_id: IssueId::new(ISSUE_ID),
        user: "admin".to_string(),
        updated_on: Some(local_datetime("2026-09-10T00:00:00+09:00")),
        details: vec![],
        notes: format!("remote notes {journal_id}"),
    }
}

fn issue(journals: Vec<Journal>) -> IssueAggregate {
    let mut issue = sample_issue_aggregate(ISSUE_ID, "issue", 1.into(), None, None, None, 0);
    issue.journals = journals;
    issue
}

fn attachment(id: u16, filename: &str) -> Attachment {
    Attachment {
        id: AttachmentId::new(id),
        filename: filename.to_string(),
        filesize: 10,
        content_type: Some("image/png".to_string()),
        description: String::new(),
        content_url: format!("/attachments/download/{id}/{filename}"),
        author: "admin".to_string(),
        created_on: local_datetime("2026-09-11T00:00:00+09:00"),
    }
}

fn loaded_store() -> Store {
    let mut store = Store::new();
    crate::test_support::load_issue(&mut store, issue(vec![journal(10)]));
    store
}

fn apply(store: &mut Store, action: impl Into<Action>) {
    store.consume_action(action.into());
}

fn start(store: &mut Store) {
    apply(
        store,
        AttachmentAction::StartUpload {
            issue_id: ISSUE_ID.into(),
            filename: "sample.png".to_string(),
        },
    );
}

fn advance_to_confirming(store: &mut Store) {
    apply(
        store,
        AttachmentAction::UploadSucceeded {
            issue_id: ISSUE_ID.into(),
        },
    );
    apply(
        store,
        AttachmentAction::AttachSucceeded {
            issue_id: ISSUE_ID.into(),
        },
    );
}

fn stage(store: &Store) -> Option<AttachmentUploadStage> {
    store
        .try_get_attachment_upload(ISSUE_ID)
        .map(|upload| upload.stage)
}

#[test]
fn upload_advances_one_stage_per_request() {
    let mut store = loaded_store();

    start(&mut store);
    assert_eq!(
        store.try_get_attachment_upload(ISSUE_ID),
        Some(&AttachmentUpload {
            filename: "sample.png".to_string(),
            stage: AttachmentUploadStage::Uploading,
        })
    );
    apply(
        &mut store,
        AttachmentAction::UploadSucceeded {
            issue_id: ISSUE_ID.into(),
        },
    );
    assert_eq!(stage(&store), Some(AttachmentUploadStage::Attaching));
    apply(
        &mut store,
        AttachmentAction::AttachSucceeded {
            issue_id: ISSUE_ID.into(),
        },
    );
    assert_eq!(stage(&store), Some(AttachmentUploadStage::Confirming));
}

#[test]
fn confirm_takes_in_fetched_attachments_and_keeps_unsaved_work() {
    let mut store = loaded_store();
    apply(
        &mut store,
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        },
    );
    apply(
        &mut store,
        JournalAction::EditRemoteNotes {
            issue_id: ISSUE_ID.into(),
            journal_id: 10.into(),
            notes: "edited notes".to_string(),
        },
    );
    start(&mut store);
    advance_to_confirming(&mut store);
    let mut fetched = issue(vec![journal(10), journal(11)]);
    fetched.attachments = vec![attachment(5, "sample.png")];

    apply(
        &mut store,
        AttachmentAction::ConfirmSucceeded {
            issue: fetched,
            children: vec![],
        },
    );

    assert_eq!(stage(&store), None);
    let (view, state) = store.get_issue(ISSUE_ID);
    assert_eq!(state, IssueState::Edited);
    assert_eq!(view.description(), "edited body");
    assert_eq!(
        view.attachments()
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<Vec<_>>(),
        vec!["sample.png"]
    );
    assert!(matches!(
        store.get_remote_journal(ISSUE_ID, 10).state,
        RemoteJournalState::Edited { .. }
    ));
    assert!(matches!(
        store.get_remote_journal(ISSUE_ID, 11).state,
        RemoteJournalState::Synced
    ));
}

#[test]
fn failure_removes_the_upload_at_any_stage() {
    let mut store = loaded_store();
    start(&mut store);
    advance_to_confirming(&mut store);

    apply(
        &mut store,
        AttachmentAction::FailUpload {
            issue_id: ISSUE_ID.into(),
        },
    );

    assert_eq!(stage(&store), None);
    // 失敗後は次の追加を始められる。
    start(&mut store);
    assert_eq!(stage(&store), Some(AttachmentUploadStage::Uploading));
}

#[test]
#[should_panic(
    expected = "cannot advance attachment upload of issue 1 from Attaching at Some(Uploading)"
)]
fn skipping_a_stage_is_rejected() {
    let mut store = loaded_store();
    start(&mut store);

    apply(
        &mut store,
        AttachmentAction::AttachSucceeded {
            issue_id: ISSUE_ID.into(),
        },
    );
}

#[test]
#[should_panic(expected = "cannot confirm attachment upload of issue 1 at Some(Attaching)")]
fn confirming_before_attaching_is_rejected() {
    let mut store = loaded_store();
    start(&mut store);
    apply(
        &mut store,
        AttachmentAction::UploadSucceeded {
            issue_id: ISSUE_ID.into(),
        },
    );

    apply(
        &mut store,
        AttachmentAction::ConfirmSucceeded {
            issue: issue(vec![journal(10)]),
            children: vec![],
        },
    );
}

#[test]
#[should_panic(expected = "cannot start attachment upload while issue 1 is Unregistered")]
fn starting_on_an_unloaded_issue_is_rejected() {
    let mut store = Store::new();

    start(&mut store);
}

#[test]
#[should_panic(
    expected = "cannot start attachment upload while another attachment of issue 1 is uploading"
)]
fn starting_a_second_upload_is_rejected() {
    let mut store = loaded_store();
    start(&mut store);

    start(&mut store);
}

#[test]
#[should_panic(expected = "cannot start attachment upload while issue 1 is Uploading")]
fn starting_during_issue_upload_is_rejected() {
    let mut store = loaded_store();
    apply(
        &mut store,
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        },
    );
    apply(
        &mut store,
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        },
    );

    start(&mut store);
}

#[test]
#[should_panic(expected = "cannot start attachment upload while a journal of issue 1 is uploading")]
fn starting_during_journal_upload_is_rejected() {
    let mut store = loaded_store();
    apply(
        &mut store,
        JournalAction::CreateLocal {
            issue_id: ISSUE_ID.into(),
        },
    );
    apply(
        &mut store,
        JournalAction::StartLocalUpload {
            issue_id: ISSUE_ID.into(),
        },
    );

    start(&mut store);
}

#[test]
#[should_panic(expected = "cannot start issue upload while an attachment of issue 1 is uploading")]
fn issue_upload_during_attachment_upload_is_rejected() {
    let mut store = loaded_store();
    apply(
        &mut store,
        IssueAction::UpdateDescription {
            id: ISSUE_ID.into(),
            body: "edited body".to_string(),
        },
    );
    start(&mut store);

    apply(
        &mut store,
        IssueAction::StartUpload {
            id: ISSUE_ID.into(),
        },
    );
}

#[test]
#[should_panic(
    expected = "cannot start journal upload while an attachment of issue 1 is uploading"
)]
fn journal_upload_during_attachment_upload_is_rejected() {
    let mut store = loaded_store();
    apply(
        &mut store,
        JournalAction::CreateLocal {
            issue_id: ISSUE_ID.into(),
        },
    );
    start(&mut store);

    apply(
        &mut store,
        JournalAction::StartLocalUpload {
            issue_id: ISSUE_ID.into(),
        },
    );
}
