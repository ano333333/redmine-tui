use std::path::PathBuf;

use crate::support::{
    open_issue_from_initial_popup, redmine_get_json, reseed_redmine, spawn_app, wait_for_redmine,
    wait_for_text,
};

// upload.rsと同じく、幅120の端末でIssue 3を表示した状態で数えた。
// 本文の後にある添付ファイルの追加行は、Journal 1のnotesへ進む51回から、
// Journal 1のdetail、子一覧の2件、追加行の次の行へ進む4回を引いた位置にある。
const J_PRESSES_TO_ADD_ATTACHMENT_ROW: usize = 47;

// Scenario: 添付ファイルの追加行でファイル名とパスを入力すると、Redmineに添付され一覧に表示される
#[test]
fn adding_a_local_file_attaches_it_to_the_issue() {
    // Given 添付するローカルファイルがあり、Issue 3の添付ファイルの追加行にfocusしている
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("add_attachment");
    std::fs::create_dir_all(&dir).expect("failed to create the attachment directory");
    let source = dir.join("source.txt");
    std::fs::write(&source, "attached by e2e").expect("failed to write the attachment");
    reseed_redmine();
    let mut session = spawn_app();
    open_issue_from_initial_popup(&mut session, 1);
    for _ in 0..J_PRESSES_TO_ADD_ATTACHMENT_ROW {
        session.press_key("j").expect("failed to press j");
    }

    // When Enterで開いたpopupに、送る名前と読み込むパスを入力して確定する
    session.press_key("Enter").expect("failed to press Enter");
    wait_for_text(&mut session, "ファイルを添付");
    session
        .write_bytes(b"e2e-note.txt")
        .expect("failed to type the filename");
    session.press_key("Tab").expect("failed to press Tab");
    session
        .write_bytes(source.display().to_string().as_bytes())
        .expect("failed to type the path");
    session.press_key("Enter").expect("failed to press Enter");

    // Then RedmineのIssue 3に入力した名前で添付され、確認の取得後に一覧へ表示される
    wait_for_redmine(
        "/issues/3.json?include=attachments",
        "attaching the local file",
        |issue| {
            issue["issue"]["attachments"]
                .as_array()
                .is_some_and(|attachments| {
                    attachments.iter().any(|attachment| {
                        attachment["filename"] == "e2e-note.txt" && attachment["filesize"] == 15
                    })
                })
        },
    );
    // seedのIssue 3は添付ファイル1件を持つため、追加後は2件になる。
    wait_for_text(&mut session, "e2e-note.txt");
    wait_for_text(&mut session, "添付ファイル 2");
}

// Scenario: Redmineの既定の上限（5120 KB）を超えるファイルは、読み込まずに追加を中止する
#[test]
fn adding_a_file_over_the_upload_limit_is_rejected_before_sending() {
    // Given 上限を1バイト超えるローカルファイルがあり、Issue 3の添付ファイルの追加行にfocusしている
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("add_large_attachment");
    std::fs::create_dir_all(&dir).expect("failed to create the attachment directory");
    let source = dir.join("large.bin");
    std::fs::write(&source, vec![0_u8; 5120 * 1024 + 1]).expect("failed to write the attachment");
    reseed_redmine();
    let mut session = spawn_app();
    open_issue_from_initial_popup(&mut session, 1);
    for _ in 0..J_PRESSES_TO_ADD_ATTACHMENT_ROW {
        session.press_key("j").expect("failed to press j");
    }

    // When popupにファイル名とパスを入力して確定する
    session.press_key("Enter").expect("failed to press Enter");
    wait_for_text(&mut session, "ファイルを添付");
    session
        .write_bytes(b"large.bin")
        .expect("failed to type the filename");
    session.press_key("Tab").expect("failed to press Tab");
    session
        .write_bytes(source.display().to_string().as_bytes())
        .expect("failed to type the path");
    session.press_key("Enter").expect("failed to press Enter");

    // Then 読み込めなかったことが通知され、RedmineのIssue 3にはseedの添付ファイルしかない
    // 通知は端末幅の1/3で折り返すため、1行目に収まる部分で待つ。
    wait_for_text(&mut session, "添付するファイル");
    let issue = redmine_get_json("/issues/3.json?include=attachments");
    let filenames: Vec<_> = issue["issue"]["attachments"]
        .as_array()
        .expect("attachments should be an array")
        .iter()
        .map(|attachment| attachment["filename"].clone())
        .collect();
    assert_eq!(filenames, vec![serde_json::json!("sample.png")]);
}
