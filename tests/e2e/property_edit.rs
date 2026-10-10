use testty::session::PtySession;

use crate::support::{
    open_issue_from_initial_popup, press_keys, property_value, reseed_redmine, spawn_app,
    wait_for_text, wait_until,
};

/// 詳細画面のproperty行。左列が0〜7行、右列が8〜14行で、右列へは同じ段の左列からlで移る。
const RIGHT_COLUMN_FIRST_LINE: usize = 8;
const TRACKER_LINE: usize = 4;
const PRIORITY_LINE: usize = 5;
const PROJECT_LINE: usize = 6;
const START_DATE_LINE: usize = 9;
const ESTIMATED_HOURS_LINE: usize = 12;
const CATEGORY_LINE: usize = 14;

const NONE_CHOICE: &str = "(None)";

/// 初期popupの`position`番目のIssueの詳細を開き、`line`のpropertyでeを押す。
///
/// 最初のjでheaderからpropertyへ移り、以降のjで1行ずつ下がる。親Issueを持つIssueでは、
/// 最初のjでheaderの親の行へ移るため、1回多く押す。
fn press_e_on_property(position: usize, has_parent: bool, line: usize) -> PtySession {
    reseed_redmine();
    let mut session = spawn_app();
    open_issue_from_initial_popup(&mut session, position);
    let (row, move_right) = if line < RIGHT_COLUMN_FIRST_LINE {
        (line, false)
    } else {
        (line - RIGHT_COLUMN_FIRST_LINE, true)
    };
    let presses = row + 1 + usize::from(has_parent);
    for _ in 0..presses {
        session.press_key("j").expect("failed to press j");
    }
    if move_right {
        session.press_key("l").expect("failed to press l");
    }
    session.press_key("e").expect("failed to press e");
    session
}

/// 子Issueを持ち、親Issueを持たないIssue 3で`line`のpropertyの編集popupを開く。
fn open_property_popup_of_issue_3(line: usize) -> PtySession {
    press_e_on_property(1, false, line)
}

/// 子Issueを持たず、Issue 3を親に持つIssue 2で`line`のpropertyの編集popupを開く。
///
/// 子Issueを持つIssueの優先度・開始日・期日・進捗率は子から計算され、編集できないため、Issue 2を使う。
fn open_property_popup_of_issue_2(line: usize) -> PtySession {
    press_e_on_property(2, true, line)
}

fn wait_for_property(session: &mut PtySession, label: &str, expected: &str) {
    wait_until(session, &format!("{label} becoming {expected}"), |frame| {
        frame.lines().any(|line| line.contains(label)) && property_value(frame, label) == expected
    });
}

// Scenario: trackerのpopupは選択なしを含まず、選んだtrackerに変わる
#[test]
fn tracker_popup_excludes_none_and_changes_the_tracker() {
    // Given Issue 3（tracker: Support）のtracker popupを開いている
    let mut session = open_property_popup_of_issue_3(TRACKER_LINE);

    // Then popupにtrackerが並び、選択なしは含まれない
    let frame = wait_for_text(&mut session, "Feature");
    assert!(frame.contains("Bug"));
    assert!(!frame.contains(NONE_CHOICE));

    // When SupportからひとつずつBugの方へ戻り、Featureを選ぶ
    press_keys(&mut session, &["k", "Enter"]);

    // Then trackerがFeatureになる
    wait_for_property(&mut session, "トラッカー", "Feature");
}

// Scenario: priorityのpopupは選択なしを含まず、選んだpriorityに変わる
#[test]
fn priority_popup_excludes_none_and_changes_the_priority() {
    // Given Issue 2（priority: major）のpriority popupを開いている
    let mut session = open_property_popup_of_issue_2(PRIORITY_LINE);

    // Then popupにpriorityが並び、選択なしは含まれない
    let frame = wait_for_text(&mut session, "blocker");
    assert!(!frame.contains(NONE_CHOICE));

    // When majorの次のminorを選ぶ
    press_keys(&mut session, &["j", "Enter"]);

    // Then priorityがminorになる
    wait_for_property(&mut session, "優先度", "minor");
}

// Scenario: 子Issueを持つIssueの優先度は、popupを開かずに編集できない理由を表示する
#[test]
fn priority_of_issue_with_children_shows_notice_without_popup() {
    // Given 子Issueを持つIssue 3（priority: major）の優先度でeを押した
    let mut session = open_property_popup_of_issue_3(PRIORITY_LINE);

    // Then 編集できない理由が表示され、priority popupは開かない
    // 通知は右上の狭い枠で折り返されるため、1行に収まる先頭部分で待つ。
    let frame = wait_for_text(&mut session, "優先度は");
    assert!(!frame.contains("blocker"));
}

// Scenario: projectを移すと、project固有のcategoryが外れる
#[test]
fn project_popup_moves_the_issue_and_clears_the_category() {
    // Given Issue 3（project: Sample Project、category: category1）のproject popupを開いている
    let mut session = open_property_popup_of_issue_3(PROJECT_LINE);
    wait_for_text(&mut session, "Sample Project 2");

    // When Sample Project 2を選ぶ
    press_keys(&mut session, &["j", "Enter"]);

    // Then projectが変わり、categoryが外れる
    wait_for_property(&mut session, "プロジェクト", "Sample Project 2");
    wait_for_property(&mut session, "カテゴリー", "-");
}

// Scenario: 予定工数を数値で入力できる
#[test]
fn estimated_hours_popup_changes_the_estimated_hours() {
    // Given Issue 3（予定工数: なし）の予定工数popupを開いている
    let mut session = open_property_popup_of_issue_3(ESTIMATED_HOURS_LINE);

    // When 2.5を入力して確定する
    press_keys(&mut session, &["2", ".", "5", "Enter"]);

    // Then 予定工数が2.5 hになる
    wait_for_property(&mut session, "予定工数", "2.5 h");
}

// Scenario: categoryのpopupは選択なしを含み、選ぶとcategoryが外れる
#[test]
fn category_popup_includes_none_and_clears_the_category() {
    // Given Issue 3（category: category1）のcategory popupを開いている
    let mut session = open_property_popup_of_issue_3(CATEGORY_LINE);

    // Then popupに選択なしとproject 1のcategoryが並ぶ
    let frame = wait_for_text(&mut session, NONE_CHOICE);
    assert!(frame.contains("category2"));
    assert!(!frame.contains("p2-category1"));

    // When category1の上にある選択なしを選ぶ
    press_keys(&mut session, &["k", "Enter"]);

    // Then categoryが外れる
    wait_for_property(&mut session, "カテゴリー", "-");
}

// Scenario: 開始日と期日を日付選択popupで変更できる
#[test]
fn date_picker_changes_the_start_date_and_the_due_date() {
    // Given Issue 2（開始日: 2025/12/09）の開始日popupを開いている
    let mut session = open_property_popup_of_issue_2(START_DATE_LINE);

    // When カレンダーで翌日を選び、入力欄で確定する
    // Tabで年・月・日の欄を越えてカレンダーボタンへ移り、Enterでカレンダーを開く。
    // lで1日進めてEnterで入力欄へ反映し、kで年の欄へ戻ってEnterで確定する。
    let pick_next_day = ["Tab", "Tab", "Tab", "Enter", "l", "Enter", "k", "Enter"];
    press_keys(&mut session, &pick_next_day);

    // Then 開始日が翌日になる
    wait_for_property(&mut session, "開始日", "2025/12/10");

    // When 期日（2025/12/19）のpopupを開き、翌日を選ぶ
    press_keys(&mut session, &["j", "e"]);
    press_keys(&mut session, &pick_next_day);

    // Then 期日が翌日になる
    wait_for_property(&mut session, "期日", "2025/12/20");
}
