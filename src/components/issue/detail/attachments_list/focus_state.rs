use ratatui::layout::Position;

use super::widget::HEADER_LINES;
use crate::platform::input::{InputEvent, KeyCode};
use crate::widgets::gutter::GUTTER_WIDTH;

pub enum FocusEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
    /// 末尾の追加行でEnterが押された。
    AddRequested,
    Handled,
}

/// 添付ファイルの行、追加中の行、末尾の追加行のうち、どの行にカーソルがあるかを持つ。
///
/// 追加行は常にあるため、行数は1以上になる。
pub struct FocusState {
    row_count: usize,
    focused_index: Option<usize>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            row_count: 1,
            focused_index: None,
        }
    }

    /// 行が減ってカーソル位置がなくなった場合は、末尾の行へ移す。
    pub fn update(&mut self, row_count: usize) {
        self.row_count = row_count;
        self.focused_index = self
            .focused_index
            .map(|index| index.min(row_count.saturating_sub(1)));
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focused_index = match event {
            FocusEvent::Unfocused => None,
            FocusEvent::CursorEnteredFromAbove => Some(0),
            FocusEvent::CursorEnteredFromBelow => Some(self.row_count.saturating_sub(1)),
        };
    }

    pub fn process_event(&mut self, event: &InputEvent) -> Option<EventProcessResult> {
        let focused_index = self.focused_index?;
        let InputEvent::Key(key) = event;
        match key.code {
            KeyCode::Char('j') => {
                if focused_index + 1 >= self.row_count {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                self.focused_index = Some(focused_index + 1);
                Some(EventProcessResult::Handled)
            }
            KeyCode::Char('k') => {
                if focused_index == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                self.focused_index = Some(focused_index - 1);
                Some(EventProcessResult::Handled)
            }
            KeyCode::Enter if focused_index + 1 == self.row_count => {
                Some(EventProcessResult::AddRequested)
            }
            _ => None,
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        Position {
            x: GUTTER_WIDTH,
            y: HEADER_LINES + self.focused_index.unwrap_or(0) as u16,
        }
    }

    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    #[test]
    fn cursor_moves_through_rows_and_leaves_at_both_ends() {
        let mut state = FocusState::new();
        state.update(2);
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::Handled)
        ));
        assert_eq!(state.focused_index(), Some(1));
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        state.process_event(&key_event(KeyCode::Char('k')));
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::CursorLeavedFromAbove)
        ));
    }

    #[test]
    fn enter_requests_an_addition_only_on_the_last_row() {
        let mut state = FocusState::new();
        state.update(2);
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert!(state.process_event(&key_event(KeyCode::Enter)).is_none());
        state.process_event(&key_event(KeyCode::Char('j')));
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Enter)),
            Some(EventProcessResult::AddRequested)
        ));
    }

    #[test]
    fn entering_from_below_focuses_the_last_row() {
        let mut state = FocusState::new();
        state.update(3);

        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        assert_eq!(state.focused_index(), Some(2));
    }

    #[test]
    fn removed_rows_move_the_cursor_to_the_last_row() {
        let mut state = FocusState::new();
        state.update(3);
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        state.update(1);

        assert_eq!(state.focused_index(), Some(0));
    }
}
