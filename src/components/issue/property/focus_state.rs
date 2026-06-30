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
}

pub struct FocusState {
    focused_y: Option<u16>,
}

impl FocusState {
    pub fn new() -> Self {
        Self { focused_y: None }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        if let Event::Key(key) = event
            && let Some(focused_y) = &mut self.focused_y
        {
            match key.code {
                KeyCode::Char('j') => {
                    if *focused_y + 1 == LINE_COUNT {
                        return Some(EventProcessResult::CursorLeavedFromBelow);
                    }
                    *focused_y += 1;
                }
                KeyCode::Char('k') => {
                    if *focused_y == 0 {
                        return Some(EventProcessResult::CursorLeavedFromAbove);
                    }
                    *focused_y -= 1;
                }
                KeyCode::Char('e') => {
                    if *focused_y == 3 {
                        return Some(EventProcessResult::OpenIssueStatusPopup);
                    }
                }
                _ => {}
            }
        }
        None
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
}
