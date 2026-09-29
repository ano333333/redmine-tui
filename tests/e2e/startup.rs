use crate::support::{reseed_redmine, spawn_app, wait_for_text};

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
