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

enum Action {
    MoveLeft,
    MoveRight,
    MoveDown,
    MoveUp,
    Edit,
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
        let action = self.action_from_event(event)?;
        self.apply_action(action)
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
        issue.description.hash(&mut hasher);
        let body_hash = hasher.finish();
        if self.width != width || self.body_hash != body_hash {
            self.body = issue.description.clone();
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

    fn action_from_event(&self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };

        self.cursor_position?;

        match key.code {
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('e') => Some(Action::Edit),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        let cursor = self.cursor_position?;

        match action {
            Action::MoveLeft => {
                self.cursor_position = Some(Position {
                    x: cursor.x.saturating_sub(1),
                    y: cursor.y,
                });
                None
            }
            Action::MoveRight => {
                if cursor.x + 1 < self.width {
                    self.cursor_position = Some(Position {
                        x: cursor.x + 1,
                        y: cursor.y,
                    });
                }
                None
            }
            Action::MoveDown => {
                if cursor.y + 1 >= self.height {
                    return Some(EventProcessResult::CursorLeavedFromBelow { x: cursor.x });
                }
                self.cursor_position = Some(Position {
                    x: cursor.x,
                    y: cursor.y + 1,
                });
                None
            }
            Action::MoveUp => {
                if cursor.y == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove { x: cursor.x });
                }
                self.cursor_position = Some(Position {
                    x: cursor.x,
                    y: cursor.y - 1,
                });
                None
            }
            Action::Edit => Some(EventProcessResult::EditRequested {
                id: self.id,
                body: self.body.clone(),
            }),
        }
    }
}
