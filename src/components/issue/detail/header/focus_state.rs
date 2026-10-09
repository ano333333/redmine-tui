use ratatui::layout::Position;

use crate::platform::input::{InputEvent, KeyCode};

pub enum FocusEvent {
    Focused,
    Unfocused,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromBelow,
    Handled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedRow {
    Title,
    Parent,
}

enum Action {
    MoveDown,
    MoveUp,
}

pub struct FocusState {
    focused_row: Option<FocusedRow>,
    title_position: Position,
    /// 親Issueがなければ`None`。親の行にはこのときフォーカスしない。
    parent_position: Option<Position>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            focused_row: None,
            title_position: Position::default(),
            parent_position: None,
        }
    }

    pub fn update(&mut self, title_position: Position, parent_position: Option<Position>) {
        self.title_position = title_position;
        self.parent_position = parent_position;
        if self.focused_row == Some(FocusedRow::Parent) && parent_position.is_none() {
            self.focused_row = Some(FocusedRow::Title);
        }
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focused_row = match event {
            FocusEvent::Focused => Some(FocusedRow::Title),
            FocusEvent::CursorEnteredFromBelow => Some(self.bottom_row()),
            FocusEvent::Unfocused => None,
        };
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    pub fn get_cursor_position(&self) -> Position {
        match self.focused_row {
            Some(FocusedRow::Parent) => self.parent_position.unwrap_or(self.title_position),
            Some(FocusedRow::Title) | None => self.title_position,
        }
    }

    pub fn focused_row(&self) -> Option<FocusedRow> {
        self.focused_row
    }

    fn bottom_row(&self) -> FocusedRow {
        if self.parent_position.is_some() {
            FocusedRow::Parent
        } else {
            FocusedRow::Title
        }
    }

    fn action_from_event(&self, event: InputEvent) -> Option<Action> {
        self.focused_row?;

        let InputEvent::Key(key) = event;
        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match (action, self.focused_row?) {
            (Action::MoveDown, row) if row == self.bottom_row() => {
                self.focused_row = None;
                Some(EventProcessResult::CursorLeavedFromBelow)
            }
            (Action::MoveDown, _) => {
                self.focused_row = Some(FocusedRow::Parent);
                Some(EventProcessResult::Handled)
            }
            (Action::MoveUp, FocusedRow::Parent) => {
                self.focused_row = Some(FocusedRow::Title);
                Some(EventProcessResult::Handled)
            }
            // ヘッダーより上にはフォーカス先がない
            (Action::MoveUp, FocusedRow::Title) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{InputEvent, KeyEvent, KeyModifiers};

    const TITLE: Position = Position { x: 4, y: 0 };
    const PARENT: Position = Position { x: 11, y: 1 };

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn state_with_parent() -> FocusState {
        let mut state = FocusState::new();
        state.update(TITLE, Some(PARENT));
        state
    }

    #[test]
    fn process_event_ignores_key_when_unfocused() {
        let mut state = FocusState::new();

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.focused_row(), None);
    }

    #[test]
    fn process_event_j_on_title_without_parent_returns_leave_result_and_clears_focus() {
        let mut state = FocusState::new();
        state.update(TITLE, None);
        state.focus_event(FocusEvent::Focused);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert_eq!(state.focused_row(), None);
    }

    #[test]
    fn process_event_j_and_k_move_between_title_and_parent() {
        let mut state = state_with_parent();
        state.focus_event(FocusEvent::Focused);

        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::Handled)
        ));
        assert_eq!(state.focused_row(), Some(FocusedRow::Parent));
        assert_eq!(state.get_cursor_position(), PARENT);

        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::Handled)
        ));
        assert_eq!(state.focused_row(), Some(FocusedRow::Title));
        assert_eq!(state.get_cursor_position(), TITLE);
    }

    #[test]
    fn process_event_j_on_parent_returns_leave_result_and_clears_focus() {
        let mut state = state_with_parent();
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert_eq!(state.focused_row(), None);
    }

    #[test]
    fn process_event_k_on_title_is_not_handled_and_keeps_focus() {
        let mut state = state_with_parent();
        state.focus_event(FocusEvent::Focused);

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        assert_eq!(state.focused_row(), Some(FocusedRow::Title));
    }

    #[test]
    fn cursor_entered_from_below_focuses_parent_only_when_it_exists() {
        let mut with_parent = state_with_parent();
        with_parent.focus_event(FocusEvent::CursorEnteredFromBelow);
        assert_eq!(with_parent.focused_row(), Some(FocusedRow::Parent));

        let mut without_parent = FocusState::new();
        without_parent.update(TITLE, None);
        without_parent.focus_event(FocusEvent::CursorEnteredFromBelow);
        assert_eq!(without_parent.focused_row(), Some(FocusedRow::Title));
    }

    #[test]
    fn update_moves_focus_to_title_when_parent_disappears() {
        let mut state = state_with_parent();
        state.focus_event(FocusEvent::CursorEnteredFromBelow);

        state.update(TITLE, None);

        assert_eq!(state.focused_row(), Some(FocusedRow::Title));
        assert_eq!(state.get_cursor_position(), TITLE);
    }

    #[test]
    fn unfocused_clears_focus() {
        let mut state = state_with_parent();
        state.focus_event(FocusEvent::Focused);

        state.focus_event(FocusEvent::Unfocused);

        assert_eq!(state.focused_row(), None);
    }
}
