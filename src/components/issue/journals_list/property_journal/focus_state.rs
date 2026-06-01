use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

use crate::components::issue::journals_list::focus_state::{
    ChildEventProcessResult, ChildFocusEvent,
};

#[derive(Clone)]
pub struct FocusState {
    line_count: u16,
    focused: bool,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            line_count: 0,
            focused: false,
        }
    }

    pub fn update(&mut self, line_count: u16) {
        self.line_count = line_count;
        if self.line_count == 0 {
            self.focused = false;
        }
    }

    pub fn focus_event(&mut self, event: ChildFocusEvent) {
        match event {
            ChildFocusEvent::Focused { .. }
            | ChildFocusEvent::CursorEnteredFromAbove
            | ChildFocusEvent::CursorEnteredFromBelow => {
                self.focused = self.line_count > 0;
            }
            ChildFocusEvent::Unfocused => {
                self.focused = false;
            }
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Option<ChildEventProcessResult> {
        if !self.focused || self.line_count == 0 {
            return None;
        }

        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => Some(ChildEventProcessResult::CursorLeavedFromBelow),
            KeyCode::Char('k') => Some(ChildEventProcessResult::CursorLeavedFromAbove),
            _ => None,
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        Position::new(0, 0)
    }
}
