use crate::support::{
    FakeEditor, open_issue_from_initial_popup, redmine_get_json, reseed_redmine, spawn_app,
    wait_for_redmine, wait_for_text, wait_until,
};

/// 新規作成画面のヒント行。作成画面が閉じたことは、これが画面から消えたことで判定する。
const NEW_ISSUE_HINT: &str = "Ctrl+S: 作成";
// Issue 4の詳細で子Issueの追加行へ移るjの回数。editing.rsのLocal Journal作成ボタンの1つ手前。
const J_PRESSES_TO_ISSUE_4_ADD_CHILD_ROW: usize = 12;

// Scenario: Issue選択popupからIssueを新規作成すると、作成したIssueの詳細が開く
#[test]
fn creating_an_issue_from_the_issue_select_popup_opens_the_created_issue() {
    // Given Issue選択popupでSample Projectにフォーカスしている
    reseed_redmine();
    let editor = FakeEditor::new("create_issue");
    let mut session = editor.spawn_app("e2e created issue");
    wait_for_text(&mut session, "issue2");

    // When nで新規作成を開き、題名をeditorで入力してctrl+sで作成する
    session.press_key("n").expect("failed to press n");
    wait_for_text(&mut session, NEW_ISSUE_HINT);
    session.press_key("Enter").expect("failed to press Enter");
    assert_eq!(editor.finish_editing(&mut session), "");
    wait_for_text(&mut session, "e2e created issue");
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then 新規作成画面が閉じて作成したIssueの詳細が開き、RedmineにIssueが作られている
    wait_until(&mut session, "opening the created issue", |frame| {
        frame.contains("e2e created issue") && !frame.contains(NEW_ISSUE_HINT)
    });
    let issues = redmine_get_json("/issues.json?project_id=1&status_id=*");
    assert!(
        issues["issues"]
            .as_array()
            .expect("issues should be an array")
            .iter()
            .any(|issue| issue["subject"] == "e2e created issue"),
        "the created issue is not in Redmine: {issues:#}"
    );
}

// Scenario: 子一覧の追加行から子Issueを新規作成すると、親Issueの子として作られる
#[test]
fn creating_a_child_from_the_children_list_creates_it_under_the_parent() {
    // Given Issue 4の詳細で子Issueの追加行にフォーカスしている
    reseed_redmine();
    let editor = FakeEditor::new("create_child_issue");
    let mut session = editor.spawn_app("e2e created child");
    open_issue_from_initial_popup(&mut session, 0);
    for _ in 0..J_PRESSES_TO_ISSUE_4_ADD_CHILD_ROW {
        session.press_key("j").expect("failed to press j");
    }

    // When Enterで子の新規作成を開き、題名をeditorで入力してctrl+sで作成する
    session.press_key("Enter").expect("failed to press Enter");
    wait_for_text(&mut session, "#4 issue4");
    session.press_key("Enter").expect("failed to press Enter");
    editor.finish_editing(&mut session);
    wait_for_text(&mut session, "e2e created child");
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then 作成した子の詳細が開き、RedmineではIssue 4の子になっている
    wait_until(&mut session, "opening the created child", |frame| {
        frame.contains("e2e created child") && !frame.contains(NEW_ISSUE_HINT)
    });
    wait_for_redmine(
        "/issues/4.json?include=children",
        "creating the child under issue 4",
        |issue| {
            issue["issue"]["children"]
                .as_array()
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| child["subject"] == "e2e created child")
                })
        },
    );
}

// Scenario: 題名のない下書きは作成せず、題名が必要なことを知らせる
#[test]
fn creating_a_draft_without_subject_tells_that_the_subject_is_required() {
    // Given Issue選択popupから新規作成を開いている
    reseed_redmine();
    let mut session = spawn_app();
    wait_for_text(&mut session, "issue2");
    session.press_key("n").expect("failed to press n");
    wait_for_text(&mut session, NEW_ISSUE_HINT);

    // When 題名を入力せずにctrl+sを押す
    session.press_key("ctrl+s").expect("failed to press ctrl+s");

    // Then 題名が必要なことが通知され、新規作成画面のままである
    let frame = wait_for_text(&mut session, "Enter a subject");
    assert!(frame.contains(NEW_ISSUE_HINT));
}
