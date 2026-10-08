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

/// Issue 3の詳細を開き、`line`のpropertyでeを押して編集popupを開く。
fn open_property_popup_of_issue_3(line: usize) -> PtySession {
    reseed_redmine();
    let mut session = spawn_app();
    open_issue_from_initial_popup(&mut session, 1);
    // 最初のjでheaderからpropertyへ移り、以降のjで1行ずつ下がる。
    let (row, move_right) = if line < RIGHT_COLUMN_FIRST_LINE {
        (line, false)
    } else {
        (line - RIGHT_COLUMN_FIRST_LINE, true)
    };
    for _ in 0..=row {
        session.press_key("j").expect("failed to press j");
    }
    if move_right {
        session.press_key("l").expect("failed to press l");
    }
    session.press_key("e").expect("failed to press e");
    session
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
    // Given Issue 3（priority: major）のpriority popupを開いている
    let mut session = open_property_popup_of_issue_3(PRIORITY_LINE);

    // Then popupにpriorityが並び、選択なしは含まれない
    let frame = wait_for_text(&mut session, "blocker");
    assert!(!frame.contains(NONE_CHOICE));

    // When majorの次のminorを選ぶ
    press_keys(&mut session, &["j", "Enter"]);

    // Then priorityがminorになる
    wait_for_property(&mut session, "優先度", "minor");
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
    // Given Issue 3（開始日: 2026/02/16）の開始日popupを開いている
    let mut session = open_property_popup_of_issue_3(START_DATE_LINE);

    // When カレンダーで翌日を選び、入力欄で確定する
    // Tabで年・月・日の欄を越えてカレンダーボタンへ移り、Enterでカレンダーを開く。
    // lで1日進めてEnterで入力欄へ反映し、kで年の欄へ戻ってEnterで確定する。
    let pick_next_day = ["Tab", "Tab", "Tab", "Enter", "l", "Enter", "k", "Enter"];
    press_keys(&mut session, &pick_next_day);

    // Then 開始日が翌日になる
    wait_for_property(&mut session, "開始日", "2026/02/17");

    // When 期日（2026/02/17）のpopupを開き、翌日を選ぶ
    press_keys(&mut session, &["j", "e"]);
    press_keys(&mut session, &pick_next_day);

    // Then 期日が翌日になる
    wait_for_property(&mut session, "期日", "2026/02/18");
}
