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
            FocusEvent::Focused { .. } => {
                self.focused = true;
            }
            FocusEvent::Unfocused => {
                self.focused = false;
            }
        }
    }

    pub fn process_event(&mut self, event: crossterm::event::Event) -> Option<EventProcessResult> {
        if self.focused
            && let Event::Key(key) = event
            && key.code == KeyCode::Char('j')
        {
            return Some(EventProcessResult::CursorLeavedFromBelow);
        }
        None
    }

    pub fn get_cursor_position(&self) -> Position {
        Position { x: 2, y: 2 }
    }
}
