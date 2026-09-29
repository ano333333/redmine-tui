use crate::support::{
    press_keys, reseed_redmine, spawn_app, spawn_app_with_api_key, wait_for_exit, wait_for_text,
};

// Scenario: 起動するとIssue選択popupにprojectとIssueが表示される
#[test]
fn startup_shows_issue_select_popup_with_seeded_issues() {
    // Given seedのproject 1にIssue 1〜3がある
    reseed_redmine();

    // When 初期Issueを指定せずに起動する
    let mut session = spawn_app();

    // Then Issue選択popupにproject名とIssueのsubjectが表示される
    let frame = wait_for_text(&mut session, "issue2");
    assert!(frame.contains("Sample Project"));
    assert!(frame.contains("issue1"));

    session.press_key("q").expect("failed to close the popup");
}

// Scenario: API keyを設定せずに起動すると、理由を表示して終了する
#[test]
fn startup_without_api_key_reports_the_missing_variable_and_exits() {
    // Given REDMINE_API_KEYが設定されていない
    reseed_redmine();

    // When 起動する
    let mut session = spawn_app_with_api_key(None);

    // Then 不足している環境変数名を表示して、失敗として終了する
    wait_for_text(&mut session, "REDMINE_API_KEY is not set");
    assert!(!wait_for_exit(&mut session));
}

// Scenario: 誤ったAPI keyで起動すると、初期データの取得失敗を表示して終了する
#[test]
fn startup_with_invalid_api_key_reports_the_initial_load_failure_and_exits() {
    // Given Redmineに存在しないAPI keyを使う
    reseed_redmine();

    // When 起動する
    let mut session = spawn_app_with_api_key(Some("invalid-api-key"));

    // Then 初期データの取得に失敗したことを表示して、失敗として終了する
    let frame = wait_for_text(&mut session, "failed to load initial entities from Redmine");
    assert!(frame.contains("401"));
    assert!(!wait_for_exit(&mut session));
}

// Scenario: popupを閉じた画面でqを押すとアプリが終了する
#[test]
fn q_on_the_main_screen_exits_the_application() {
    // Given Issue選択popupを表示している
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");

    // When popupを閉じてから、もう一度qを押す
    press_keys(&mut session, &["q", "q"]);

    // Then アプリが正常終了する
    assert!(wait_for_exit(&mut session));
}
