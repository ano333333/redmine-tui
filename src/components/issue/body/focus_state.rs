use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;
use std::cmp::min;

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove { x: u16 },
    CursorEnteredFromBelow { x: u16 },
}

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
}

pub struct FocusState {
    width: u16,
    height: u16,
    cursor_position: Option<Position>,
}

impl FocusState {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cursor_position: None,
        }
    }

    // Widgetの占める領域を、componentのupdate時に更新する
    pub fn update(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        if let Some(cursor_position) = &mut self.cursor_position {
            cursor_position.x = cursor_position.x.min(width);
            cursor_position.y = cursor_position.y.min(height);
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        if let Some(cursor) = self.cursor_position
            && let Event::Key(key) = event
        {
            match key.code {
                KeyCode::Char('h') => {
                    self.cursor_position = Some(Position {
                        x: cursor.x.saturating_sub(1),
                        y: cursor.y,
                    });
                }
                KeyCode::Char('l') => {
                    if cursor.x + 1 < self.width {
                        self.cursor_position = Some(Position {
                            x: cursor.x + 1,
                            y: cursor.y,
                        });
                    }
                }
                KeyCode::Char('j') => {
                    if cursor.y + 1 >= self.height {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: cursor.x });
                    }
                    self.cursor_position = Some(Position {
                        x: cursor.x,
                        y: cursor.y + 1,
                    });
                }
                KeyCode::Char('k') => {
                    if cursor.y == 0 {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: cursor.x });
                    }
                    self.cursor_position = Some(Position {
                        x: cursor.x,
                        y: cursor.y - 1,
                    });
                }
                _ => {}
            }
        }
        None
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::Focused {
                position: Position { x, y },
            } => {
                self.cursor_position = Some(Position {
                    x: min(x, self.width.saturating_sub(1)),
                    y: min(y, self.height.saturating_sub(1)),
                });
            }
            FocusEvent::Unfocused => {
                self.cursor_position = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                self.cursor_position = Some(Position {
                    x: min(x, self.width.saturating_sub(1)),
                    y: 0,
                });
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                self.cursor_position = Some(Position {
                    x: min(x, self.width.saturating_sub(1)),
                    y: self.height.saturating_sub(1),
                });
            }
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        self.cursor_position.unwrap()
    }

    pub fn is_focused(&self) -> bool {
        self.cursor_position.is_some()
    }
}
