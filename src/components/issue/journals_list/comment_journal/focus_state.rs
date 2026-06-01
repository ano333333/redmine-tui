use std::cmp::min;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Offset, Position};

use crate::components::issue::journals_list::focus_state::{
    ChildEventProcessResult, ChildFocusEvent,
};

#[derive(Clone)]
pub struct FocusState {
    width: u16,
    body_line_count: u16,
    cursor_position: Option<CursorPosition>,
}

#[derive(Clone)]
enum CursorPosition {
    Header,
    Body { position: Position },
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            width: 0,
            body_line_count: 0,
            cursor_position: None,
        }
    }

    pub fn update(&mut self, width: u16, body_line_count: u16) {
        self.width = width;
        self.body_line_count = body_line_count;
        let Some(cursor_position) = &mut self.cursor_position else {
            return;
        };

        match cursor_position {
            CursorPosition::Header => {}
            CursorPosition::Body { position } => {
                position.x = min(position.x, width.saturating_sub(1));
                if body_line_count == 0 {
                    self.cursor_position = Some(CursorPosition::Header);
                    return;
                }
                position.y = min(position.y, body_line_count - 1);
            }
        }
    }

    pub fn focus_event(&mut self, event: ChildFocusEvent) {
        match event {
            ChildFocusEvent::Focused { position } => {
                if position.y < 2 || self.body_line_count == 0 {
                    self.cursor_position = Some(CursorPosition::Header);
                } else {
                    self.cursor_position = Some(CursorPosition::Body {
                        position: Position {
                            x: min(position.x, self.width.saturating_sub(1)),
                            y: min(position.y - 2, self.body_line_count - 1),
                        },
                    });
                }
            }
            ChildFocusEvent::Unfocused => {
                self.cursor_position = None;
            }
            ChildFocusEvent::CursorEnteredFromAbove => {
                self.cursor_position = Some(CursorPosition::Header);
            }
            ChildFocusEvent::CursorEnteredFromBelow => {
                if self.body_line_count == 0 {
                    self.cursor_position = Some(CursorPosition::Header);
                } else {
                    self.cursor_position = Some(CursorPosition::Body {
                        position: Position::new(0, self.body_line_count - 1),
                    });
                }
            }
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Option<ChildEventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };

        match &mut self.cursor_position {
            Some(CursorPosition::Header) => match key.code {
                KeyCode::Char('j') => {
                    if self.body_line_count == 0 {
                        return Some(ChildEventProcessResult::CursorLeavedFromBelow);
                    }
                    self.cursor_position = Some(CursorPosition::Body {
                        position: Position::new(0, 0),
                    });
                }
                KeyCode::Char('k') => {
                    return Some(ChildEventProcessResult::CursorLeavedFromAbove);
                }
                _ => {}
            },
            Some(CursorPosition::Body { position }) => match key.code {
                KeyCode::Char('j') => {
                    if position.y + 1 >= self.body_line_count {
                        return Some(ChildEventProcessResult::CursorLeavedFromBelow);
                    }
                    position.y += 1;
                }
                KeyCode::Char('k') => {
                    if position.y == 0 {
                        self.cursor_position = Some(CursorPosition::Header);
                    } else {
                        position.y -= 1;
                    }
                }
                KeyCode::Char('h') => {
                    position.x = position.x.saturating_sub(1);
                }
                KeyCode::Char('l') => {
                    if position.x + 1 < self.width {
                        position.x += 1;
                    }
                }
                _ => {}
            },
            None => {}
        }

        None
    }

    pub fn get_cursor_position(&self) -> Position {
        match self.cursor_position {
            Some(CursorPosition::Header) => Position::new(0, 0),
            Some(CursorPosition::Body { position }) => position + Offset { x: 0, y: 2 },
            None => Position::default(),
        }
    }
}
