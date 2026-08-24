use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

use crate::vos::IssueId;

pub enum FocusEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
}

pub struct FocusState {
    ids: Vec<IssueId>,
    focused_id: Option<IssueId>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            ids: vec![],
            focused_id: None,
        }
    }

    pub fn update(&mut self, ids: &[IssueId]) {
        let Some(focused_id) = self.focused_id else {
            self.ids = ids.to_vec();
            return;
        };

        let new_index = if let Some(pos) = ids.iter().position(|&id| id == focused_id) {
            // フォーカス中のIDがまだ存在する -> そのインデックスを維持
            pos
        } else {
            // なくなっていれば同じインデックス（末尾にclamp）
            let old_index = self
                .ids
                .iter()
                .position(|&id| id == focused_id)
                .unwrap_or(0);
            old_index.min(ids.len().saturating_sub(1))
        };

        self.ids = ids.to_vec();
        self.focused_id = ids.get(new_index).copied();
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::Unfocused => {
                self.focused_id = None;
            }
            FocusEvent::CursorEnteredFromAbove => {
                self.focused_id = self.ids.first().copied();
            }
            FocusEvent::CursorEnteredFromBelow => {
                self.focused_id = self.ids.last().copied();
            }
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Option<EventProcessResult> {
        let focused_id = self.focused_id?;
        let Event::Key(key) = event else {
            return None;
        };
        let focused_index = self.ids.iter().position(|&id| id == focused_id)?;

        match key.code {
            KeyCode::Char('j') => {
                if focused_index + 1 == self.ids.len() {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                self.focused_id = self.ids.get(focused_index + 1).copied();
            }
            KeyCode::Char('k') => {
                if focused_index == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                self.focused_id = self.ids.get(focused_index - 1).copied();
            }
            _ => {}
        }
        None
    }

    pub fn get_cursor_position(&self) -> Position {
        let focused_id = self.focused_id.unwrap();
        let index = self
            .ids
            .iter()
            .position(|&id| id == focused_id)
            .unwrap_or(0);
        Position {
            x: 0,
            y: index as u16 + 2,
        }
    }

    pub fn focused_index(&self) -> Option<usize> {
        let focused_id = self.focused_id?;
        self.ids.iter().position(|&id| id == focused_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ids(values: &[u16]) -> Vec<IssueId> {
        values.iter().copied().map(Into::into).collect()
    }

    #[test]
    fn focus_from_above_after_update_focuses_first_id() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));

        state.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert_eq!(state.focused_index(), Some(0));
        assert_eq!(state.get_cursor_position(), Position { x: 0, y: 2 });
    }

    #[test]
    fn focus_from_below_after_update_focuses_last_id() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));

        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        assert_eq!(state.focused_index(), Some(2));
        assert_eq!(state.get_cursor_position(), Position { x: 0, y: 4 });
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

        assert!(
            state
                .process_event(&key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(state.focused_index(), Some(1));

        assert!(
            state
                .process_event(&key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn process_event_j_on_last_item_returns_leave_from_below_and_keeps_focus() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        let result = state.process_event(&key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert_eq!(state.focused_index(), Some(2));
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

        state.update(&ids(&[4]));

        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    #[ignore = "未実装: update([]) 時に focus を明示的に解除する仕様として固定したい"]
    fn update_empty_ids_clears_focus() {
        let mut state = FocusState::new();
        state.update(&ids(&[1, 2, 3]));
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        state.update(&[]);

        assert_eq!(state.focused_index(), None);
    }
}
