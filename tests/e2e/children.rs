use crate::support::{open_issue_from_initial_popup, reseed_redmine, spawn_app, wait_until};

// upload.rsと同じく、幅120の端末でIssue 3を表示した状態で数えた。子一覧はこの直前にあり、画面に入る。
const J_PRESSES_TO_FIRST_JOURNAL_NOTES: usize = 47;

// Scenario: 子一覧は直下の子を表示し、孫以下の件数を添え、詳細を開いていない子は未取得と表示する
#[test]
fn the_children_list_shows_direct_children_with_their_descendant_counts() {
    // Given seedでは、Issue 3の子であるIssue 1の下に孫のIssue 4がある
    reseed_redmine();
    let mut session = spawn_app();

    // When Issue 3を開き、子一覧の下まで移る
    open_issue_from_initial_popup(&mut session, 1);
    for _ in 0..J_PRESSES_TO_FIRST_JOURNAL_NOTES {
        session.press_key("j").expect("failed to press j");
    }

    // Then Issue 1の行に孫の件数が添えられ、孫自身の行はない
    let frame = wait_until(&mut session, "showing the children list", |frame| {
        frame.contains("issue1 (+1)")
    });
    assert!(frame.contains("未取得"));
    assert!(!frame.contains("issue4"));
}
