use testty::session::PtySession;

use crate::support::{
    FakeEditor, open_issue_from_initial_popup, press_keys, reseed_redmine, wait_for_text,
};

// Issue 3の詳細でheaderから下へ移るjの回数。幅120の端末で、seedのIssue 3とJournal 1〜3を表示した状態で数えた。
// propertyの左列8行を越える9回目で本文の先頭に入る。
const J_PRESSES_TO_BODY: usize = 9;
// 本文35行、子Issue一覧2行、Journal 1のdetail 1行を越えた先がJournal 1のnotes。
const J_PRESSES_TO_FIRST_JOURNAL_NOTES: usize = 47;
// Journal 3のnotesの後にあるLocal Journal作成ボタン。
const J_PRESSES_TO_CREATE_LOCAL_JOURNAL_BUTTON: usize = 85;

fn open_issue_3_and_move_down(
    editor: &FakeEditor,
    edited_text: &str,
    presses: usize,
) -> PtySession {
    reseed_redmine();
    let mut session = editor.spawn_app(edited_text);
    open_issue_from_initial_popup(&mut session, 0);
    for _ in 0..presses {
        session.press_key("j").expect("failed to press j");
    }
    session
}

// Scenario: 本文をeditorで編集すると、詳細に編集後の本文が表示される
#[test]
fn editing_the_issue_body_shows_the_edited_body() {
    // Given Issue 3の本文にfocusしている
    let editor = FakeEditor::new("edit_body");
    let mut session = open_issue_3_and_move_down(&editor, "edited body marker", J_PRESSES_TO_BODY);

    // When eでeditorを開き、本文を書き換える
    session.press_key("e").expect("failed to press e");
    let initial_text = editor.finish_editing(&mut session);

    // Then editorには元の本文が渡され、詳細に編集後の本文が表示される
    // seedのIssue 3の本文は見出しで始まり、引用で終わるMarkdownである。
    assert!(initial_text.starts_with("### h3"));
    assert!(initial_text.ends_with("> citation"));
    wait_for_text(&mut session, "edited body marker");
}

// Scenario: Remote Journalのnotesをeditorで編集すると、編集後のnotesが表示される
#[test]
fn editing_remote_journal_notes_shows_the_edited_notes() {
    // Given Issue 3のJournal 1（notesは空）にfocusしている
    let editor = FakeEditor::new("edit_remote_journal");
    let mut session = open_issue_3_and_move_down(
        &editor,
        "edited journal notes",
        J_PRESSES_TO_FIRST_JOURNAL_NOTES,
    );

    // When eでeditorを開き、notesを書き換える
    session.press_key("e").expect("failed to press e");
    let initial_text = editor.finish_editing(&mut session);

    // Then editorには空のnotesが渡され、編集後のnotesが表示される
    assert_eq!(initial_text, "");
    wait_for_text(&mut session, "edited journal notes");
}

// Scenario: Local Journalを作成してnotesを書き、もう一度editorで開ける
#[test]
fn creating_a_local_journal_opens_the_editor_and_keeps_the_notes() {
    // Given Issue 3のLocal Journal作成ボタンにfocusしている
    let editor = FakeEditor::new("create_local_journal");
    let mut session = open_issue_3_and_move_down(
        &editor,
        "local journal notes",
        J_PRESSES_TO_CREATE_LOCAL_JOURNAL_BUTTON,
    );

    // When Enterでボタンを押す
    session.press_key("Enter").expect("failed to press Enter");
    let initial_text = editor.finish_editing(&mut session);

    // Then 空のnotesでeditorが開き、書いたnotesがLocal Journalとして表示される
    assert_eq!(initial_text, "");
    wait_for_text(&mut session, "local journal notes");

    // When 作成したLocal Journalのnotesをeで開き直す
    // 最下段は作成ボタンのままなので、jで最下段にいることを確かめてからkで1つ上のnotesへ移る。
    press_keys(&mut session, &["j", "k", "e"]);

    // Then editorには保存済みのnotesが渡される
    assert_eq!(editor.finish_editing(&mut session), "local journal notes");
}
