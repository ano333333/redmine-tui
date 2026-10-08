use reqwest::{Method, StatusCode};

use crate::support::{press_keys, redmine_api, reseed_redmine, spawn_app, wait_for_text};

// Scenario: Issueの取得に失敗すると理由を表示し、rで再試行できる
#[test]
fn issue_fetch_failure_shows_the_reason_and_r_retries() {
    // Given Issue選択popupにIssue 2が表示されている
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");
    // And 一覧の取得後に、Issue 2がRedmineから削除されている
    assert_eq!(
        redmine_api(Method::DELETE, "/issues/2.json", None),
        StatusCode::NO_CONTENT
    );

    // When 一覧の2番目にあるIssue 2を開く
    // 一覧はID降順（4, 3, 2, 1）で並び、lでIssue一覧へ移ってjで3番目を選ぶ。
    press_keys(&mut session, &["l", "j", "j", "Enter"]);

    // Then 404による取得失敗が表示される
    wait_for_text(&mut session, "404");

    // When Issue 2を復元してからrを押す
    reseed_redmine();
    session.press_key("r").expect("failed to press r");

    // Then Issue 2の詳細が表示される
    let frame = wait_for_text(&mut session, "Feature");
    assert!(frame.contains("issue2"));
    assert!(!frame.contains("404"));
}
