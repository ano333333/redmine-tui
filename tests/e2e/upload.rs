use testty::session::PtySession;

use crate::support::{
    FakeEditor, open_issue_from_initial_popup, press_keys, property_value, redmine_api,
    reseed_redmine, spawn_app, wait_for_redmine, wait_for_text, wait_until,
};

// headerから下へ移るjの回数。editing.rsと同じく、幅120の端末でseedのIssueを表示した状態で数えた。
// propertyの優先度は、最初のjでpropertyへ入った後、5行下にある。
const J_PRESSES_TO_PRIORITY: usize = 6;
// propertyの左列8行を越える9回目で本文の先頭に入る。
const J_PRESSES_TO_BODY: usize = 9;
const J_PRESSES_TO_FIRST_JOURNAL_NOTES: usize = 47;
const J_PRESSES_TO_CREATE_LOCAL_JOURNAL_BUTTON: usize = 85;

const ISSUE_3_WITH_JOURNALS: &str = "/issues/3.json?include=journals";

fn press_j(session: &mut PtySession, presses: usize) {
    for _ in 0..presses {
        session.press_key("j").expect("failed to press j");
    }
}

fn journal_notes(issue: &serde_json::Value, journal_id: u64) -> Option<String> {
    issue["issue"]["journals"]
        .as_array()?
        .iter()
        .find(|journal| journal["id"] == journal_id)
        .and_then(|journal| journal["notes"].as_str())
        .map(str::to_string)
}

// Scenario: 編集したIssueをctrl+sでuploadすると、Redmineに反映される
#[test]
fn ctrl_s_uploads_the_edited_issue() {
    // Given Issue 2（priority: major）の優先度をminorに変えている
    // 子Issueを持つIssue 3の優先度は子から計算され、Redmineが更新を無視するため、Issue 2を使う。
    reseed_redmine();
    let mut session = spawn_app();
    open_issue_from_initial_popup(&mut session, 1);
    press_j(&mut session, J_PRESSES_TO_PRIORITY);
    press_keys(&mut session, &["e", "j", "Enter"]);
    wait_until(&mut session, "changing the priority to minor", |frame| {
        property_value(frame, "優先度") == "minor"
    });

    // When journal一覧の外でctrl+sを押す
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then RedmineのIssue 2の優先度がminor（ID 2）になる
    wait_for_redmine("/issues/2.json", "uploading the priority", |issue| {
        issue["issue"]["priority"]["id"] == 2
    });
}

// Scenario: 編集したRemote Journalのnotesをctrl+sでuploadすると、Redmineに反映される
#[test]
fn ctrl_s_uploads_the_edited_remote_journal_notes() {
    // Given Issue 3のJournal 1のnotesを編集している
    let editor = FakeEditor::new("upload_remote_journal");
    reseed_redmine();
    let mut session = editor.spawn_app("uploaded journal notes");
    open_issue_from_initial_popup(&mut session, 0);
    press_j(&mut session, J_PRESSES_TO_FIRST_JOURNAL_NOTES);
    session.press_key("e").expect("failed to press e");
    editor.finish_editing(&mut session);
    // editorの結果を反映するまでは編集中として入力が無視されるため、表示を待つ。
    wait_for_text(&mut session, "uploaded journal notes");

    // When そのJournalでctrl+sを押す
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then RedmineのJournal 1のnotesが編集後の内容になる
    wait_for_redmine(ISSUE_3_WITH_JOURNALS, "uploading journal 1", |issue| {
        journal_notes(issue, 1).as_deref() == Some("uploaded journal notes")
    });
}

// Scenario: Local Journalをctrl+sでuploadすると、Redmineに新しいJournalとして作成される
#[test]
fn ctrl_s_uploads_the_local_journal_as_a_new_journal() {
    // Given Issue 3にLocal Journalを作成してnotesを書いている
    let editor = FakeEditor::new("upload_local_journal");
    reseed_redmine();
    let mut session = editor.spawn_app("uploaded local notes");
    open_issue_from_initial_popup(&mut session, 0);
    press_j(&mut session, J_PRESSES_TO_CREATE_LOCAL_JOURNAL_BUTTON);
    session.press_key("Enter").expect("failed to press Enter");
    editor.finish_editing(&mut session);
    wait_for_text(&mut session, "uploaded local notes");

    // When Local Journalのnotesでctrl+sを押す
    // 最下段は作成ボタンのままなので、1つ上のnotesへ移る。
    press_keys(&mut session, &["j", "k", "ctrl+s"]);

    // Then RedmineのIssue 3に、seedのJournal 1〜3に続くJournal 4として作成される
    wait_for_redmine(
        ISSUE_3_WITH_JOURNALS,
        "uploading the local journal",
        |issue| journal_notes(issue, 4).as_deref() == Some("uploaded local notes"),
    );
}

// Scenario: サーバーから消えた編集中のJournalは退避され、ctrl+sで新しいJournalとして投稿される
#[test]
fn ctrl_s_posts_an_evacuated_journal_as_a_new_journal() {
    // Given Issue 3のJournal 3のnotesを編集した後、Redmine上でJournal 3が削除されている
    // detailを持たないJournalはnotesを空にすると削除される。
    let editor = FakeEditor::new("post_evacuated_journal");
    reseed_redmine();
    let mut session = editor.spawn_app("rescued notes");
    open_issue_from_initial_popup(&mut session, 0);
    // 作成ボタンの1つ上がJournal 3のnotesの最終行。
    press_j(&mut session, J_PRESSES_TO_CREATE_LOCAL_JOURNAL_BUTTON);
    press_keys(&mut session, &["k", "e"]);
    editor.finish_editing(&mut session);
    wait_for_text(&mut session, "rescued notes");
    assert_eq!(
        redmine_api(
            reqwest::Method::PUT,
            "/journals/3.json",
            Some(serde_json::json!({ "journal": { "notes": "" } })),
        ),
        reqwest::StatusCode::NO_CONTENT
    );

    // When Local Journalを投稿して取得結果を取り込み、退避された項目でctrl+sを押す
    press_keys(&mut session, &["j", "Enter"]);
    editor.finish_editing(&mut session);
    // editorの結果を反映するまでは編集中として入力が無視されるため、表示を待つ。
    wait_for_text(&mut session, "(local)");
    press_keys(&mut session, &["j", "k", "ctrl+s"]);
    // Local Journalが消えると、同じ位置にある退避の項目へfocusが移る。
    wait_for_text(&mut session, "(deleted #3)");
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then seedのJournal 1〜3に続き、Local JournalがJournal 4、退避したnotesがJournal 5として作成される
    wait_for_redmine(
        ISSUE_3_WITH_JOURNALS,
        "posting the evacuated journal",
        |issue| {
            journal_notes(issue, 3).is_none()
                && journal_notes(issue, 5).as_deref() == Some("rescued notes")
        },
    );
}

// Scenario: 編集中のJournalがサーバーで書き換えられていても、Issueの保存は続き、Journalの編集は送られずに残る
#[test]
fn issue_upload_continues_and_keeps_an_edited_journal_changed_on_the_server() {
    // Given Issue 3のJournal 1のnotesと本文を編集した後、Redmine上でJournal 1のnotesが書き換えられている
    let editor = FakeEditor::new("issue_upload_with_journal_conflict");
    reseed_redmine();
    let mut session = editor.spawn_app("local text");
    open_issue_from_initial_popup(&mut session, 0);
    press_j(&mut session, J_PRESSES_TO_FIRST_JOURNAL_NOTES);
    session.press_key("e").expect("failed to press e");
    editor.finish_editing(&mut session);
    wait_for_text(&mut session, "(edited)");
    for _ in 0..J_PRESSES_TO_FIRST_JOURNAL_NOTES - J_PRESSES_TO_BODY {
        session.press_key("k").expect("failed to press k");
    }
    session.press_key("e").expect("failed to press e");
    editor.finish_editing(&mut session);
    wait_for_text(&mut session, "(edited)");
    assert_eq!(
        redmine_api(
            reqwest::Method::PUT,
            "/journals/1.json",
            Some(serde_json::json!({ "journal": { "notes": "server text" } })),
        ),
        reqwest::StatusCode::NO_CONTENT
    );

    // When 本文でctrl+sを押してIssueを保存する
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then 本文は保存され、Journal 1はサーバーの値のまま、画面では編集中として残る
    wait_for_redmine(ISSUE_3_WITH_JOURNALS, "uploading the issue body", |issue| {
        issue["issue"]["description"] == "local text"
            && journal_notes(issue, 1).as_deref() == Some("server text")
    });
    wait_for_text(&mut session, "(edited)");
}
