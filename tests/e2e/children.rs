use crate::support::{open_issue_from_initial_popup, reseed_redmine, spawn_app, wait_until};

// upload.rsと同じく、幅120の端末でIssue 3を表示した状態で数えた。子一覧はこの直前にあり、画面に入る。
const J_PRESSES_TO_FIRST_JOURNAL_NOTES: usize = 47;
// 子一覧の2件目、Journal一覧の先頭行、Notesへ進む3回を引くと、子一覧の先頭のIssue 1に止まる。
const J_PRESSES_TO_FIRST_CHILD: usize = J_PRESSES_TO_FIRST_JOURNAL_NOTES - 3;

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

// Scenario: 子一覧でフォーカス中の子をEnterで開く
#[test]
fn enter_on_a_child_opens_that_child_issue() {
    // Given seedでは、Issue 3の子であるIssue 1の下に孫のIssue 4がある
    reseed_redmine();
    let mut session = spawn_app();

    // When Issue 3を開き、子一覧の先頭のIssue 1でEnterを押す
    open_issue_from_initial_popup(&mut session, 1);
    for _ in 0..J_PRESSES_TO_FIRST_CHILD {
        session.press_key("j").expect("failed to press j");
    }
    session.press_key("Enter").expect("failed to press Enter");

    // Then Issue 1の詳細が開き、その子一覧にIssue 4が出る
    wait_until(
        &mut session,
        "opening issue 1 from the children list",
        |frame| frame.contains("issue4"),
    );
}

// Scenario: 親Issueを読み込んでいなくても、題名の下に親Issueの題名を表示する
#[test]
fn the_header_shows_the_parent_issue_by_fetching_it() {
    // Given seedでは、Issue 1の親がIssue 3である
    reseed_redmine();
    let mut session = spawn_app();

    // When Issue 3を開かずに、Issue 1を開く
    open_issue_from_initial_popup(&mut session, 3);

    // Then 題名の下に、取得したIssue 3の題名が出る。seedのIssue 3の題名は「issue1(長…)」で始まる
    wait_until(&mut session, "showing the parent issue", |frame| {
        frame.contains("親チケット #3 issue1(長")
    });
}
