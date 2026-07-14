use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

const LINE_COUNT: u16 = 14;

pub enum FocusEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
    OpenIssueStatusPopup,
    OpenSpentTimeInputPopup,
}

enum Action {
    MoveDown,
    MoveUp,
    OpenIssueStatusPopup,
    OpenSpentTimeInputPopup,
}

pub struct FocusState {
    focused_y: Option<u16>,
}

impl FocusState {
    pub fn new() -> Self {
        Self { focused_y: None }
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
                self.focused_y = Some(LINE_COUNT - 1);
            }
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        Position {
            x: 20,
            y: self.focused_y.unwrap_or(0),
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
            KeyCode::Char('e') if focused_y == 3 => Some(Action::OpenIssueStatusPopup),
            KeyCode::Char('a') if focused_y == LINE_COUNT - 1 => {
                Some(Action::OpenSpentTimeInputPopup)
            }
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::MoveDown => {
                let focused_y = self.focused_y.as_mut()?;
                if *focused_y + 1 == LINE_COUNT {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                *focused_y += 1;
                None
            }
            Action::MoveUp => {
                let focused_y = self.focused_y.as_mut()?;
                if *focused_y == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                *focused_y -= 1;
                None
            }
            Action::OpenIssueStatusPopup => Some(EventProcessResult::OpenIssueStatusPopup),
            Action::OpenSpentTimeInputPopup => Some(EventProcessResult::OpenSpentTimeInputPopup),
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
    }

    #[test]
    fn process_event_e_on_issue_status_line_opens_popup() {
        let mut state = FocusState { focused_y: Some(3) };

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenIssueStatusPopup)
        ));
        assert_eq!(state.focused_y(), Some(3));
    }

    #[test]
    fn process_event_a_on_last_line_opens_spent_time_popup() {
        let mut state = FocusState {
            focused_y: Some(LINE_COUNT - 1),
        };

        let result = state.process_event(key_event(KeyCode::Char('a')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenSpentTimeInputPopup)
        ));
        assert_eq!(state.focused_y(), Some(LINE_COUNT - 1));
    }
}
