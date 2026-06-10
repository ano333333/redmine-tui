use super::widget::{BodyWidget, BodyWidgetState};
use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;
use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::entities::Issue;

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove { x: u16 },
    CursorEnteredFromBelow { x: u16 },
}

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
    EditRequested { id: u16, body: String },
}

pub struct BodyComponent {
    id: u16,
    width: u16,
    height: u16,
    body_hash: u64,
    cursor_position: Option<Position>,
    body: String,
    widget_state: BodyWidgetState,
}

impl BodyComponent {
    pub fn new(id: u16, width: u16, height: u16) -> Self {
        Self {
            id,
            width,
            height,
            body_hash: 0,
            cursor_position: None,
            body: String::new(),
            widget_state: BodyWidgetState::new(),
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
                KeyCode::Char('e') => {
                    return Some(EventProcessResult::EditRequested {
                        id: self.id,
                        body: self.body.clone(),
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

    pub fn update(&mut self, issue: &Issue, width: u16) {
        let mut hasher = DefaultHasher::new();
        issue.body.hash(&mut hasher);
        let body_hash = hasher.finish();
        if self.width != width || self.body_hash != body_hash {
            self.body = issue.body.clone();
            self.body_hash = body_hash;
            self.widget_state.update(width, &self.body);
        }
        self.width = width;
        self.height = self.widget_state.line_count(width) as u16;
        if let Some(cursor_position) = &mut self.cursor_position {
            cursor_position.x = cursor_position.x.min(self.width);
            cursor_position.y = cursor_position.y.min(self.height);
        }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.widget_state.line_count(width) as u16
    }

    pub fn create_widget<'a>(&'a self) -> BodyWidget<'a> {
        BodyWidget::new(&self.widget_state, self.cursor_position.is_some())
    }

    pub fn get_cursor_position(&self) -> Position {
        self.cursor_position.unwrap_or(Position::new(0, 0))
    }
}
