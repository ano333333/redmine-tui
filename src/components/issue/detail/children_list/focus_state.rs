use ratatui::layout::Position;

use super::widget::HEADER_LINES;
use crate::widgets::gutter::GUTTER_WIDTH;

use crate::platform::input::{InputEvent, KeyCode};
use crate::vos::IssueId;

pub enum FocusEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
    OpenRequested { id: IssueId },
    CreateRequested,
    Handled,
}

/// 子Issueの行と末尾の追加行のうち、どの行にカーソルがあるか。
#[derive(Clone, Copy)]
enum FocusedRow {
    Child(IssueId),
    AddRow,
}

/// 追加行は常にあるため、子Issueがなくてもカーソルを置ける。
pub struct FocusState {
    ids: Vec<IssueId>,
    focused_row: Option<FocusedRow>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            ids: vec![],
            focused_row: None,
        }
    }

    pub fn update(&mut self, ids: &[IssueId]) {
        let Some(FocusedRow::Child(focused_id)) = self.focused_row else {
            self.ids = ids.to_vec();
            return;
        };

        let new_row = if ids.contains(&focused_id) {
            FocusedRow::Child(focused_id)
        } else {
            // 再取得で対象が消えても、近い行から操作を続けられるよう表示位置を保つ。
            let old_index = self
                .ids
                .iter()
                .position(|&id| id == focused_id)
                .unwrap_or(0);
            ids.get(old_index.min(ids.len().saturating_sub(1)))
                .map_or(FocusedRow::AddRow, |&id| FocusedRow::Child(id))
        };

        self.ids = ids.to_vec();
        self.focused_row = Some(new_row);
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focused_row = match event {
            FocusEvent::Unfocused => None,
            FocusEvent::CursorEnteredFromAbove => Some(self.row_at(0)),
            FocusEvent::CursorEnteredFromBelow => Some(FocusedRow::AddRow),
        };
    }

    pub fn process_event(&mut self, event: &InputEvent) -> Option<EventProcessResult> {
        let focused_row = self.focused_row?;
        let InputEvent::Key(key) = event;
        let focused_index = self.index_of(focused_row);

        match key.code {
            KeyCode::Char('j') => {
                if matches!(focused_row, FocusedRow::AddRow) {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                self.focused_row = Some(self.row_at(focused_index + 1));
                Some(EventProcessResult::Handled)
            }
            KeyCode::Char('k') => {
                if focused_index == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                self.focused_row = Some(self.row_at(focused_index - 1));
                Some(EventProcessResult::Handled)
            }
            KeyCode::Enter => match focused_row {
                FocusedRow::Child(id) => Some(EventProcessResult::OpenRequested { id }),
                FocusedRow::AddRow => Some(EventProcessResult::CreateRequested),
            },
            _ => None,
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        let index = self.focused_index().unwrap_or(0);
        Position {
            x: GUTTER_WIDTH,
            y: index as u16 + HEADER_LINES,
        }
    }

    /// 追加行は子Issueの行の直後のインデックスになる。
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_row.map(|row| self.index_of(row))
    }

    fn index_of(&self, row: FocusedRow) -> usize {
        match row {
            FocusedRow::Child(id) => self
                .ids
                .iter()
                .position(|&child_id| child_id == id)
                .unwrap_or(0),
            FocusedRow::AddRow => self.ids.len(),
        }
    }

    fn row_at(&self, index: usize) -> FocusedRow {
        self.ids
            .get(index)
            .map_or(FocusedRow::AddRow, |&id| FocusedRow::Child(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{InputEvent, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn ids(values: &[u32]) -> Vec<IssueId> {
        values.iter().copied().map(Into::into).collect()
    }

    #[test]
    fn focus_from_above_after_update_focuses_first_id() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));

        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert_eq!(state.focused_index(), Some(0));
        // 一覧は縦線の分だけ字下げされる
        assert_eq!(
            state.get_cursor_position(),
            Position {
                x: GUTTER_WIDTH,
                y: 2
            }
        );
    }

    #[test]
    fn focus_from_below_after_update_focuses_add_row() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));

        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        assert_eq!(state.focused_index(), Some(3));
        assert_eq!(
            state.get_cursor_position(),
            Position {
                x: GUTTER_WIDTH,
                y: 5
            }
        );
    }

    #[test]
    fn without_children_cursor_moves_through_add_row_and_leaves_at_both_ends() {
        let mut state = FocusState::new();
        state.update(&[]);
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert_eq!(state.focused_index(), Some(0));
        assert_eq!(
            state.get_cursor_position(),
            Position {
                x: GUTTER_WIDTH,
                y: 2
            }
        );
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Enter)),
            Some(EventProcessResult::CreateRequested)
        ));
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::CursorLeavedFromAbove)
        ));
    }

    #[test]
    fn unfocused_clears_focus() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        state.focus_event(FocusEvent::Unfocused);

        assert_eq!(state.focused_index(), None);
    }

    #[test]
    fn process_event_j_and_k_are_ignored_when_unfocused() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));

        assert!(
            state
                .process_event(&key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert!(
            state
                .process_event(&key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(state.focused_index(), None);
    }

    #[test]
    fn process_event_j_and_k_move_to_adjacent_ids_when_focused() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::Handled)
        ));
        assert_eq!(state.focused_index(), Some(1));

        assert!(matches!(
            state.process_event(&key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::Handled)
        ));
        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn process_event_j_on_last_child_moves_to_add_row() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromBelow);
        state.process_event(&key_event(KeyCode::Char('k')));

        let result = state.process_event(&key_event(KeyCode::Char('j')));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert_eq!(state.focused_index(), Some(3));
    }

    #[test]
    fn process_event_j_on_add_row_returns_leave_from_below_and_keeps_focus() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        let result = state.process_event(&key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert_eq!(state.focused_index(), Some(3));
    }

    #[test]
    fn process_event_k_on_first_item_returns_leave_from_above_and_keeps_focus() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = state.process_event(&key_event(KeyCode::Char('k')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromAbove)
        ));
        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn process_event_enter_requests_opening_the_focused_id() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);
        state.process_event(&key_event(KeyCode::Char('j')));

        let result = state.process_event(&key_event(KeyCode::Enter));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenRequested { id }) if id == IssueId::from(2)
        ));
    }

    #[test]
    fn process_event_enter_is_ignored_when_unfocused() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));

        assert!(state.process_event(&key_event(KeyCode::Enter)).is_none());
    }

    #[test]
    fn update_keeps_focus_on_same_id_when_it_still_exists() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);
        state.process_event(&key_event(KeyCode::Char('j')));

        state.update(&ids(&[2, 9, 8]));

        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn update_keeps_old_index_when_focused_id_disappears() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);
        state.process_event(&key_event(KeyCode::Char('j')));

        state.update(&ids(&[4, 5]));

        assert_eq!(state.focused_index(), Some(1));
    }

    #[test]
    fn update_clamps_to_last_index_when_focused_id_disappears_and_new_list_is_shorter() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromBelow);
        state.process_event(&key_event(KeyCode::Char('k')));

        state.update(&ids(&[4]));

        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn update_empty_ids_moves_focus_from_child_to_add_row() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        state.update(&[]);

        assert_eq!(state.focused_index(), Some(0));
        assert!(matches!(
            state.process_event(&key_event(KeyCode::Enter)),
            Some(EventProcessResult::CreateRequested)
        ));
    }
}
