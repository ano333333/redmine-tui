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
    Edit,
}

enum Action {
    MoveLeft,
    MoveRight,
    MoveDown,
    MoveUp,
    Edit,
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

    pub fn update(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;

        if let Some(cursor_position) = &mut self.cursor_position {
            cursor_position.x = cursor_position.x.min(self.width);
            cursor_position.y = cursor_position.y.min(self.height);
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        self.cursor_position.unwrap_or(Position::new(0, 0))
    }

    pub fn is_focused(&self) -> bool {
        self.cursor_position.is_some()
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
            Action::Edit => Some(EventProcessResult::Edit),
        }
    }
}
