use reqwest::{Method, StatusCode};
use serde_json::json;

use crate::support::{
    POPUP_HEADER, press_keys, redmine_api, reseed_redmine, spawn_app, wait_for_popup_to_close,
    wait_for_text, wait_until,
};

/// Issue 3の詳細にだけ表示されるtracker名。
const ISSUE_3_TRACKER: &str = "Support";
/// Issue 2の詳細にだけ表示されるtracker名。
const ISSUE_2_TRACKER: &str = "Feature";

/// popupを開き、Issue一覧を読み込み終えるまで待つ。読み込み前のキー入力は一覧に届かない。
fn open_popup(session: &mut testty::session::PtySession) {
    session.press_key("y").expect("failed to press y");
    wait_until(session, "loading the issue list in the popup", |frame| {
        frame.contains(POPUP_HEADER) && frame.contains("issue2")
    });
}

// Scenario: Issue一覧で選択を動かすとプレビューが切り替わる
#[test]
fn moving_the_selection_updates_the_preview() {
    // Given Issue 2のdescriptionにプレビュー確認用の文字列がある
    reseed_redmine();
    assert_eq!(
        redmine_api(
            Method::PUT,
            "/issues/2.json",
            Some(json!({"issue": {"description": "second preview marker"}})),
        ),
        StatusCode::NO_CONTENT
    );
    // And Issue一覧で2番目のIssue 3を選び、そのdescriptionがプレビューされている
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");
    press_keys(&mut session, &["l", "j"]);
    wait_for_text(&mut session, "normal text");

    // When 次のIssueを選ぶ
    session.press_key("j").expect("failed to press j");

    // Then Issue 2のdescriptionがプレビューされる
    let frame = wait_for_text(&mut session, "second preview marker");
    assert!(!frame.contains("normal text"));
}

// Scenario: Issueを選んでEnterを押すと詳細が開く
#[test]
fn enter_on_the_issue_list_opens_the_issue_detail() {
    // Given Issue選択popupを表示している
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");

    // When 一覧の2番目のIssue 3を開く
    press_keys(&mut session, &["l", "j", "Enter"]);

    // Then popupが閉じ、Issue 3の詳細が表示される
    let frame = wait_for_popup_to_close(&mut session);
    assert!(frame.contains(ISSUE_3_TRACKER));
}

// Scenario: 詳細画面からpopupを開き、qで同じ詳細へ戻る
#[test]
fn y_opens_the_popup_and_q_returns_to_the_same_detail() {
    // Given Issue 3の詳細を表示している
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");
    press_keys(&mut session, &["l", "j", "Enter"]);
    wait_for_popup_to_close(&mut session);

    // When yでpopupを開く
    // Then Issue選択popupにIssue一覧が表示される
    open_popup(&mut session);

    // When qでpopupを閉じる
    session.press_key("q").expect("failed to press q");

    // Then Issue 3の詳細に戻る
    let frame = wait_for_popup_to_close(&mut session);
    assert!(frame.contains(ISSUE_3_TRACKER));
}

// Scenario: popupで別のIssueを選ぶと詳細が切り替わり、同じIssueを選ぶと詳細が保たれる
#[test]
fn selecting_an_issue_in_the_popup_switches_the_detail() {
    // Given Issue 3の詳細を表示している
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");
    press_keys(&mut session, &["l", "j", "Enter"]);
    wait_for_popup_to_close(&mut session);

    // When popupで3番目のIssue 2を選ぶ
    open_popup(&mut session);
    press_keys(&mut session, &["l", "j", "j", "Enter"]);

    // Then Issue 2の詳細に切り替わる
    let frame = wait_until(&mut session, "showing issue 2", |frame| {
        !frame.contains(POPUP_HEADER) && frame.contains(ISSUE_2_TRACKER)
    });
    assert!(!frame.contains(ISSUE_3_TRACKER));

    // When popupで表示中のIssue 2を選び直す
    open_popup(&mut session);
    press_keys(&mut session, &["l", "j", "j", "Enter"]);

    // Then Issue 2の詳細が表示されたままになる
    let frame = wait_until(&mut session, "showing issue 2 again", |frame| {
        !frame.contains(POPUP_HEADER) && frame.contains(ISSUE_2_TRACKER)
    });
    assert!(!frame.contains(ISSUE_3_TRACKER));
}

// Scenario: 起動直後のpopupをqで閉じると、Issueを表示していない画面になる
#[test]
fn q_on_the_initial_popup_shows_an_empty_main_screen() {
    // Given 起動直後のIssue選択popupを表示している
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");

    // When qでpopupを閉じる
    session.press_key("q").expect("failed to press q");

    // Then Issueの詳細を表示していない画面になる
    let frame = wait_for_popup_to_close(&mut session);
    assert!(!frame.contains("issue2"));
    assert!(!frame.contains(ISSUE_3_TRACKER));
}
