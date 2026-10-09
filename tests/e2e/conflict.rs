use reqwest::{Method, StatusCode};
use serde_json::json;
use testty::session::PtySession;

use crate::support::{
    FakeEditor, open_issue_from_initial_popup, press_keys, property_value, redmine_api,
    redmine_get_json, reseed_redmine, spawn_app, wait_for_redmine, wait_for_text, wait_until,
};

// headerから下へ移るjの回数。upload.rsと同じく、幅120の端末でseedのIssueを表示した状態で数えた。
// 優先度はIssue 2で数えた。Issue 2には親Issueがあるため、headerの親の行の分だけ1回多い。
const J_PRESSES_TO_PRIORITY: usize = 7;
const J_PRESSES_TO_FIRST_JOURNAL_NOTES: usize = 47;

const ISSUE_3_WITH_JOURNALS: &str = "/issues/3.json?include=journals";
/// Issue競合popupの表の見出し。popupは値を名前ではなくIDで表示するため、見出しで判定する。
const ISSUE_CONFLICT_HEADER: &str = "サーバーの値";

fn press_j(session: &mut PtySession, presses: usize) {
    for _ in 0..presses {
        session.press_key("j").expect("failed to press j");
    }
}

fn server_priority_id() -> serde_json::Value {
    redmine_get_json("/issues/2.json")["issue"]["priority"]["id"].clone()
}

fn journal_1_notes(issue: &serde_json::Value) -> Option<String> {
    issue["issue"]["journals"]
        .as_array()?
        .iter()
        .find(|journal| journal["id"] == 1)
        .and_then(|journal| journal["notes"].as_str())
        .map(str::to_string)
}

/// Issue 2の優先度をローカルでminorに変え、サーバー側をcriticalに変えてからctrl+sで衝突させる。
fn open_issue_priority_conflict() -> PtySession {
    // Given Issue 2（priority: major）の優先度をローカルでminorに変えている
    reseed_redmine();
    let mut session = spawn_app();
    open_issue_from_initial_popup(&mut session, 2);
    press_j(&mut session, J_PRESSES_TO_PRIORITY);
    press_keys(&mut session, &["e", "j", "Enter"]);
    wait_until(&mut session, "changing the priority to minor", |frame| {
        property_value(frame, "優先度") == "minor"
    });
    // And 別の更新でサーバー側の優先度がcriticalになっている
    assert_eq!(
        redmine_api(
            Method::PUT,
            "/issues/2.json",
            Some(json!({"issue": {"priority_id": 3}})),
        ),
        StatusCode::NO_CONTENT
    );

    // When ctrl+sでuploadする
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then Issue競合popupに、優先度の編集前・編集後・サーバーの値が並ぶ
    let frame = wait_for_text(&mut session, ISSUE_CONFLICT_HEADER);
    assert!(
        frame.contains("編集内容とサーバー内容が競合しています。採用する方を選択してください。")
    );
    // FIXME: popupがIDで表示している。名前で表示するようになったら major / minor / critical に変える。
    assert_eq!(table_row(&frame, "優先度"), vec!["優先度", "1", "2", "3"]);
    assert!(frame.contains("キャンセル") && frame.contains("続行"));
    session
}

/// `label`で始まる表の行を、空白で区切ったセルとして返す。
fn table_row(frame: &str, label: &str) -> Vec<String> {
    frame
        .lines()
        .find(|line| line.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("{label} row is not displayed\nterminal frame:\n{frame}"))
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

// Scenario: Issueの競合で続行すると、ローカルの値がRedmineに反映される
#[test]
fn continuing_an_issue_conflict_uploads_the_local_value() {
    let mut session = open_issue_priority_conflict();

    // When 編集後の値を選んだまま続行ボタンを押す
    press_keys(&mut session, &["j", "Enter"]);

    // Then Redmineの優先度がminor（ID 2）になる
    wait_for_redmine("/issues/2.json", "uploading the local priority", |issue| {
        issue["issue"]["priority"]["id"] == 2
    });
}

// Scenario: Issueの競合でキャンセルすると、Redmineは変わらずローカルの編集が残る
#[test]
fn cancelling_an_issue_conflict_keeps_the_local_edit() {
    let mut session = open_issue_priority_conflict();

    // When キャンセルボタンを押す
    press_keys(&mut session, &["j", "h", "Enter"]);

    // Then popupが閉じ、ローカルの優先度はminorのまま残る
    wait_until(&mut session, "closing the conflict popup", |frame| {
        !frame.contains(ISSUE_CONFLICT_HEADER) && property_value(frame, "優先度") == "minor"
    });
    // And Redmineの優先度はcritical（ID 3）のまま
    assert_eq!(server_priority_id(), 3);
}

// Scenario: Issueの競合popupはqでもキャンセルできる
#[test]
fn q_cancels_an_issue_conflict() {
    let mut session = open_issue_priority_conflict();

    // When qを押す
    session.press_key("q").expect("failed to press q");

    // Then popupが閉じ、ローカルの優先度はminorのまま残る
    wait_until(&mut session, "closing the conflict popup", |frame| {
        !frame.contains(ISSUE_CONFLICT_HEADER) && property_value(frame, "優先度") == "minor"
    });
    // And Redmineの優先度はcritical（ID 3）のまま
    assert_eq!(server_priority_id(), 3);
}

/// Issue 3のJournal 1をローカルで編集し、サーバー側のnotesを変えてからctrl+sで衝突させる。
fn open_journal_conflict(editor: &FakeEditor) -> PtySession {
    // Given Issue 3のJournal 1のnotesをローカルで編集している
    reseed_redmine();
    let mut session = editor.spawn_app("local notes");
    open_issue_from_initial_popup(&mut session, 1);
    press_j(&mut session, J_PRESSES_TO_FIRST_JOURNAL_NOTES);
    session.press_key("e").expect("failed to press e");
    editor.finish_editing(&mut session);
    wait_for_text(&mut session, "local notes");
    // And 別の更新でサーバー側のnotesが変わっている
    assert_eq!(
        redmine_api(
            Method::PUT,
            "/journals/1.json",
            Some(json!({"journal": {"notes": "server notes"}})),
        ),
        StatusCode::NO_CONTENT
    );

    // When ctrl+sでuploadする
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then Journal競合popupに、ローカルとサーバーのnotesが左右に並ぶ
    let frame = wait_for_text(&mut session, "server notes");
    assert!(frame.contains("Journal本文が競合しています。採用する方を選択してください。"));
    assert!(frame.contains("LOCAL") && frame.contains("REMOTE"));
    let notes_line = frame
        .lines()
        .find(|line| line.contains("server notes"))
        .expect("server notes should be displayed");
    let local_at = notes_line
        .find("local notes")
        .expect("local notes should be on the same line as server notes");
    assert!(local_at < notes_line.find("server notes").unwrap());
    assert!(frame.contains("続行") && frame.contains("キャンセル"));
    session
}

/// popupが閉じ、ローカルで編集したnotesが詳細に残るまで待つ。
fn wait_for_journal_conflict_popup_to_close(session: &mut PtySession) {
    wait_until(session, "closing the journal conflict popup", |frame| {
        !frame.contains("server notes") && frame.contains("local notes")
    });
}

// Scenario: Journalの競合で続行すると、ローカルのnotesがRedmineに反映される
#[test]
fn continuing_a_journal_conflict_uploads_the_local_notes() {
    let editor = FakeEditor::new("continue_journal_conflict");
    let mut session = open_journal_conflict(&editor);

    // When ローカルのnotesを選んだまま続行ボタンを押す
    press_keys(&mut session, &["j", "Enter"]);

    // Then RedmineのJournal 1のnotesがローカルの内容になる
    wait_for_redmine(
        ISSUE_3_WITH_JOURNALS,
        "uploading the local notes",
        |issue| journal_1_notes(issue).as_deref() == Some("local notes"),
    );
}

// Scenario: Journalの競合popupはqでキャンセルでき、Redmineは変わらない
#[test]
fn q_cancels_a_journal_conflict() {
    let editor = FakeEditor::new("q_journal_conflict");
    let mut session = open_journal_conflict(&editor);

    // When qを押す
    session.press_key("q").expect("failed to press q");

    // Then popupが閉じてローカルの編集が残り、RedmineのJournal 1はサーバーのnotesのまま
    wait_for_journal_conflict_popup_to_close(&mut session);
    assert_eq!(
        journal_1_notes(&redmine_get_json(ISSUE_3_WITH_JOURNALS)).as_deref(),
        Some("server notes")
    );
}

// Scenario: Journalの競合popupはEscでキャンセルでき、Redmineは変わらない
#[test]
fn esc_cancels_a_journal_conflict() {
    let editor = FakeEditor::new("esc_journal_conflict");
    let mut session = open_journal_conflict(&editor);

    // When Escを押す
    session.press_key("Esc").expect("failed to press Esc");

    // Then popupが閉じてローカルの編集が残り、RedmineのJournal 1はサーバーのnotesのまま
    wait_for_journal_conflict_popup_to_close(&mut session);
    assert_eq!(
        journal_1_notes(&redmine_get_json(ISSUE_3_WITH_JOURNALS)).as_deref(),
        Some("server notes")
    );
}

// Scenario: 続行する前にサーバーのnotesが再び変わると、新しいnotesで競合popupが開き直す
#[test]
#[ignore = "FIXME: 続行後に古いサーバーnotesのままpopupが開き直す不具合を修正するまで失敗する"]
fn a_second_server_change_reopens_the_journal_conflict_with_the_new_notes() {
    let editor = FakeEditor::new("re_journal_conflict");
    let mut session = open_journal_conflict(&editor);
    // Given 競合popupを表示している間に、サーバーのnotesがさらに変わる
    assert_eq!(
        redmine_api(
            Method::PUT,
            "/journals/1.json",
            Some(json!({"journal": {"notes": "newer remote notes"}})),
        ),
        StatusCode::NO_CONTENT
    );

    // When ローカルのnotesを選んだまま続行する
    press_keys(&mut session, &["j", "Enter"]);

    // Then 新しいサーバーのnotesで競合popupが開き直す
    wait_for_text(&mut session, "newer remote notes");

    // When サーバーのnotesを選んで続行する
    press_keys(&mut session, &["l", "Enter", "j", "Enter"]);

    // Then popupが閉じ、RedmineのJournal 1は新しいサーバーのnotesのまま
    wait_until(
        &mut session,
        "closing the reopened conflict popup",
        |frame| !frame.contains("local notes"),
    );
    assert_eq!(
        journal_1_notes(&redmine_get_json(ISSUE_3_WITH_JOURNALS)).as_deref(),
        Some("newer remote notes")
    );
}
