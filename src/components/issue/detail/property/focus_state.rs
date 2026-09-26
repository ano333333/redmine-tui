use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

use super::widget::PropertyWidget;
use crate::widgets::gutter::GUTTER_WIDTH;

const LINE_COUNT: u16 = 15;
const ISSUE_STATUS_LINE: u16 = 3;
const PRIORITY_LINE: u16 = 5;
const ASSIGNED_TO_LINE: u16 = 7;
const TARGET_VERSION_LINE: u16 = 8;
const START_DATE_LINE: u16 = 9;
const DUE_DATE_LINE: u16 = 10;
const DONE_RATIO_LINE: u16 = 11;
const TOTAL_SPENT_HOURS_LINE: u16 = 13;
const CATEGORY_LINE: u16 = 14;
/// 2カラム表示で右カラムの先頭になる項目インデックス。
const RIGHT_COLUMN_FIRST_LINE: u16 = LINE_COUNT.div_ceil(2);

pub enum FocusEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
    OpenIssueStatusPopup,
    OpenPriorityPopup,
    OpenAssignedToPopup,
    OpenTargetVersionPopup,
    OpenStartDatePopup,
    OpenDueDatePopup,
    OpenDoneRatioPopup,
    OpenSpentTimeInputPopup,
    OpenCategoryPopup,
}

enum Action {
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    OpenIssueStatusPopup,
    OpenPriorityPopup,
    OpenAssignedToPopup,
    OpenTargetVersionPopup,
    OpenStartDatePopup,
    OpenDueDatePopup,
    OpenDoneRatioPopup,
    OpenSpentTimeInputPopup,
    OpenCategoryPopup,
}

pub struct FocusState {
    is_two_column: bool,
    focused_y: Option<u16>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            is_two_column: false,
            focused_y: None,
        }
    }

    pub fn update(&mut self, is_two_column: bool) {
        self.is_two_column = is_two_column;
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::Unfocused => {
                self.focused_y = None;
            }
            FocusEvent::CursorEnteredFromAbove => {
                self.focused_y = Some(0);
            }
            FocusEvent::CursorEnteredFromBelow => {
                // 2カラム時は左カラムの最下行へ入る
                self.focused_y = Some(if self.is_two_column {
                    RIGHT_COLUMN_FIRST_LINE - 1
                } else {
                    LINE_COUNT - 1
                });
            }
        }
    }

    /// 2カラム表示では、フォーカス項目のインデックスを (列, 行) へ写す。
    pub fn get_cursor_position(&self, width: u16) -> Position {
        let index = self.focused_y.unwrap_or(0);

        // 左カラムの値は、縦線の字下げ分だけ右へずれる
        let left_column_x = GUTTER_WIDTH + PropertyWidget::VALUE_X;

        if !PropertyWidget::is_two_column(width) {
            return Position {
                x: left_column_x,
                y: index,
            };
        }

        let split_at = RIGHT_COLUMN_FIRST_LINE;
        if index < split_at {
            Position {
                x: left_column_x,
                y: index,
            }
        } else {
            Position {
                // right_column_x は字下げを含んだ桁を返す
                x: PropertyWidget::right_column_x(width) + PropertyWidget::VALUE_X,
                y: index - split_at,
            }
        }
    }

    pub fn focused_y(&self) -> Option<u16> {
        self.focused_y
    }

    fn action_from_event(&self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };

        let focused_y = self.focused_y?;

        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('e') if focused_y == ISSUE_STATUS_LINE => {
                Some(Action::OpenIssueStatusPopup)
            }
            KeyCode::Char('e') if focused_y == PRIORITY_LINE => Some(Action::OpenPriorityPopup),
            KeyCode::Char('e') if focused_y == ASSIGNED_TO_LINE => {
                Some(Action::OpenAssignedToPopup)
            }
            KeyCode::Char('e') if focused_y == TARGET_VERSION_LINE => {
                Some(Action::OpenTargetVersionPopup)
            }
            KeyCode::Char('e') if focused_y == START_DATE_LINE => Some(Action::OpenStartDatePopup),
            KeyCode::Char('e') if focused_y == DUE_DATE_LINE => Some(Action::OpenDueDatePopup),
            KeyCode::Char('e') if focused_y == DONE_RATIO_LINE => Some(Action::OpenDoneRatioPopup),
            KeyCode::Char('e') if focused_y == TOTAL_SPENT_HOURS_LINE => {
                Some(Action::OpenSpentTimeInputPopup)
            }
            KeyCode::Char('e') if focused_y == CATEGORY_LINE => Some(Action::OpenCategoryPopup),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::MoveDown => {
                let focused_y = self.focused_y.as_mut()?;
                let is_column_bottom = *focused_y + 1 == LINE_COUNT
                    || (self.is_two_column && *focused_y + 1 == RIGHT_COLUMN_FIRST_LINE);
                if is_column_bottom {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                *focused_y += 1;
                None
            }
            Action::MoveUp => {
                let focused_y = self.focused_y.as_mut()?;
                let is_column_top = *focused_y == 0
                    || (self.is_two_column && *focused_y == RIGHT_COLUMN_FIRST_LINE);
                if is_column_top {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                *focused_y -= 1;
                None
            }
            Action::MoveLeft => {
                if !self.is_two_column {
                    return None;
                }
                let focused_y = self.focused_y.as_mut()?;
                if *focused_y >= RIGHT_COLUMN_FIRST_LINE {
                    *focused_y -= RIGHT_COLUMN_FIRST_LINE;
                }
                None
            }
            Action::MoveRight => {
                if !self.is_two_column {
                    return None;
                }
                let focused_y = self.focused_y.as_mut()?;
                if *focused_y < RIGHT_COLUMN_FIRST_LINE {
                    // 右カラムは1行少ないので、左カラム最下行からは右カラム最下行へ寄せる
                    *focused_y = (*focused_y + RIGHT_COLUMN_FIRST_LINE).min(LINE_COUNT - 1);
                }
                None
            }
            Action::OpenIssueStatusPopup => Some(EventProcessResult::OpenIssueStatusPopup),
            Action::OpenPriorityPopup => Some(EventProcessResult::OpenPriorityPopup),
            Action::OpenAssignedToPopup => Some(EventProcessResult::OpenAssignedToPopup),
            Action::OpenTargetVersionPopup => Some(EventProcessResult::OpenTargetVersionPopup),
            Action::OpenStartDatePopup => Some(EventProcessResult::OpenStartDatePopup),
            Action::OpenDueDatePopup => Some(EventProcessResult::OpenDueDatePopup),
            Action::OpenDoneRatioPopup => Some(EventProcessResult::OpenDoneRatioPopup),
            Action::OpenSpentTimeInputPopup => Some(EventProcessResult::OpenSpentTimeInputPopup),
            Action::OpenCategoryPopup => Some(EventProcessResult::OpenCategoryPopup),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn process_event_ignores_key_when_unfocused() {
        let mut state = FocusState::new();

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.focused_y(), None);
    }

    #[test]
    fn focus_event_unfocused_clears_focus() {
        let mut state = FocusState::new();
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        state.focus_event(FocusEvent::Unfocused);

        assert_eq!(state.focused_y(), None);
    }

    /// 2カラムに畳まない幅。
    const NARROW_WIDTH: u16 = 40;
    /// 左カラムの値が始まる桁(縦線の字下げ + ラベル幅)。
    const LEFT_COLUMN_VALUE_X: u16 = GUTTER_WIDTH + PropertyWidget::VALUE_X;
    /// 2カラムに畳む幅。
    const WIDE_WIDTH: u16 = 100;

    #[test]
    fn get_cursor_position_maps_later_fields_to_the_right_column_when_wide() {
        let split_at = LINE_COUNT.div_ceil(2);

        // 分割位置の手前は左カラムに残る
        let state = FocusState {
            is_two_column: false,
            focused_y: Some(split_at - 1),
        };
        assert_eq!(
            state.get_cursor_position(WIDE_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: split_at - 1,
            }
        );

        // 分割位置の項目は右カラムの先頭へ回る
        let state = FocusState {
            is_two_column: false,
            focused_y: Some(split_at),
        };
        assert_eq!(
            state.get_cursor_position(WIDE_WIDTH),
            Position {
                x: PropertyWidget::right_column_x(WIDE_WIDTH) + PropertyWidget::VALUE_X,
                y: 0,
            }
        );

        // 最後の項目は右カラムの最下行
        let state = FocusState {
            is_two_column: false,
            focused_y: Some(LINE_COUNT - 1),
        };
        assert_eq!(
            state.get_cursor_position(WIDE_WIDTH).y,
            LINE_COUNT - 1 - split_at
        );
    }

    #[test]
    fn get_cursor_position_stays_in_one_column_when_narrow() {
        let state = FocusState {
            is_two_column: false,
            focused_y: Some(LINE_COUNT - 1),
        };

        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: LINE_COUNT - 1,
            }
        );
    }

    #[test]
    fn get_cursor_position_returns_default_when_unfocused() {
        let state = FocusState::new();

        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: 0
            }
        );
    }

    #[test]
    fn get_cursor_position_tracks_focus_event_changes() {
        let mut state = FocusState::new();

        state.focus_event(FocusEvent::CursorEnteredFromAbove);
        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: 0
            }
        );

        state.focus_event(FocusEvent::CursorEnteredFromBelow);
        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: LINE_COUNT - 1
            }
        );

        state.focus_event(FocusEvent::Unfocused);
        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: 0
            }
        );
    }

    #[test]
    fn get_cursor_position_tracks_process_event_changes() {
        let mut state = FocusState::new();
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        state.process_event(key_event(KeyCode::Char('j')));

        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: 1
            }
        );

        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        state.process_event(key_event(KeyCode::Char('k')));

        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: LINE_COUNT - 2
            }
        );
    }

    #[test]
    fn process_event_j_moves_focus_down() {
        let mut state = FocusState::new();
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.focused_y(), Some(1));
    }

    #[test]
    fn process_event_j_on_last_line_returns_leave_from_below() {
        let mut state = FocusState::new();
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert_eq!(state.focused_y(), Some(LINE_COUNT - 1));
        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: LINE_COUNT - 1
            }
        );
    }

    #[test]
    fn process_event_k_on_first_line_returns_leave_from_above() {
        let mut state = FocusState::new();
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromAbove)
        ));
        assert_eq!(state.focused_y(), Some(0));
        assert_eq!(
            state.get_cursor_position(NARROW_WIDTH),
            Position {
                x: LEFT_COLUMN_VALUE_X,
                y: 0
            }
        );
    }

    #[test]
    fn process_event_e_on_issue_status_line_opens_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(3),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenIssueStatusPopup)
        ));
        assert_eq!(state.focused_y(), Some(3));
    }

    #[test]
    fn process_event_e_on_priority_line_opens_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(PRIORITY_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenPriorityPopup)
        ));
        assert_eq!(state.focused_y(), Some(PRIORITY_LINE));
    }

    #[test]
    fn process_event_e_on_assigned_to_line_opens_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(ASSIGNED_TO_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenAssignedToPopup)
        ));
        assert_eq!(state.focused_y(), Some(ASSIGNED_TO_LINE));
    }

    #[test]
    fn process_event_e_on_target_version_line_opens_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(TARGET_VERSION_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenTargetVersionPopup)
        ));
        assert_eq!(state.focused_y(), Some(TARGET_VERSION_LINE));
    }

    #[test]
    fn process_event_e_on_start_date_line_opens_date_picker_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(START_DATE_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenStartDatePopup)
        ));
        assert_eq!(state.focused_y(), Some(START_DATE_LINE));
    }

    #[test]
    fn process_event_e_on_due_date_line_opens_date_picker_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(DUE_DATE_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(result, Some(EventProcessResult::OpenDueDatePopup)));
        assert_eq!(state.focused_y(), Some(DUE_DATE_LINE));
    }

    #[test]
    fn process_event_e_on_done_ratio_line_opens_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(DONE_RATIO_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenDoneRatioPopup)
        ));
        assert_eq!(state.focused_y(), Some(DONE_RATIO_LINE));
    }

    #[test]
    fn process_event_e_on_total_spent_hours_line_opens_spent_time_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(TOTAL_SPENT_HOURS_LINE),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenSpentTimeInputPopup)
        ));
        assert_eq!(state.focused_y(), Some(TOTAL_SPENT_HOURS_LINE));
    }

    #[test]
    fn process_event_e_on_category_line_opens_popup() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(LINE_COUNT - 1),
        };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenCategoryPopup)
        ));
        assert_eq!(state.focused_y(), Some(CATEGORY_LINE));
    }

    fn two_column_state(focused_y: u16) -> FocusState {
        let mut state = FocusState::new();
        state.update(true);
        state.focused_y = Some(focused_y);
        state
    }

    #[test]
    fn process_event_l_and_h_switch_columns_keeping_row_when_wide() {
        let mut state = two_column_state(3);

        state.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(state.focused_y(), Some(11));

        state.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(state.focused_y(), Some(3));
    }

    #[test]
    fn process_event_l_on_left_bottom_moves_to_right_bottom_when_wide() {
        let mut state = two_column_state(7);

        state.process_event(key_event(KeyCode::Char('l')));

        assert_eq!(state.focused_y(), Some(14));
    }

    #[test]
    fn process_event_h_on_left_column_and_l_on_right_column_keep_focus_when_wide() {
        let mut state = two_column_state(2);
        state.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(state.focused_y(), Some(2));

        let mut state = two_column_state(10);
        state.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(state.focused_y(), Some(10));
    }

    #[test]
    fn process_event_h_and_l_are_ignored_when_narrow() {
        let mut state = FocusState {
            is_two_column: false,
            focused_y: Some(3),
        };

        state.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(state.focused_y(), Some(3));

        state.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(state.focused_y(), Some(3));
    }

    #[test]
    fn process_event_j_on_column_bottom_returns_leave_from_below_when_wide() {
        for bottom in [7, 14] {
            let mut state = two_column_state(bottom);

            let result = state.process_event(key_event(KeyCode::Char('j')));

            assert!(matches!(
                result,
                Some(EventProcessResult::CursorLeavedFromBelow)
            ));
            assert_eq!(state.focused_y(), Some(bottom));
        }
    }

    #[test]
    fn process_event_k_on_column_top_returns_leave_from_above_when_wide() {
        for top in [0, 8] {
            let mut state = two_column_state(top);

            let result = state.process_event(key_event(KeyCode::Char('k')));

            assert!(matches!(
                result,
                Some(EventProcessResult::CursorLeavedFromAbove)
            ));
            assert_eq!(state.focused_y(), Some(top));
        }
    }

    #[test]
    fn focus_event_from_below_enters_left_column_bottom_when_wide() {
        let mut state = FocusState::new();
        state.update(true);

        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        assert_eq!(state.focused_y(), Some(7));
    }
}
