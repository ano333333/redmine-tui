use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

pub enum FocusEvent {
    Focused,
    Unfocused,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromBelow,
}

enum Action {
    LeaveFromBelow,
}

pub struct FocusState {
    focused: bool,
}

impl FocusState {
    pub fn new() -> Self {
        Self { focused: false }
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::CursorEnteredFromBelow => {
                self.focused = true;
            }
            FocusEvent::Focused => {
                self.focused = true;
            }
            FocusEvent::Unfocused => {
                self.focused = false;
            }
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    pub fn get_cursor_position(&self) -> Position {
        Position { x: 2, y: 2 }
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    fn action_from_event(&self, event: Event) -> Option<Action> {
        if !self.focused {
            return None;
        }

        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => Some(Action::LeaveFromBelow),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::LeaveFromBelow => {
                self.focused = false;
                Some(EventProcessResult::CursorLeavedFromBelow)
            }
        }
    }
}

