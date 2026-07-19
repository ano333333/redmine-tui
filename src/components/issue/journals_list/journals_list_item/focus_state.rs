use std::cmp::min;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

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

enum FocusedPosition {
    Property(usize),
    Comment(Position),
}

enum Action {
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
}

pub struct FocusState {
    width: u16,
    property_count: usize,
    comment_line_count: u16,
    focused_position: Option<FocusedPosition>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            width: 0,
            property_count: 0,
            comment_line_count: 0,
            focused_position: None,
        }
    }

    pub fn update(&mut self, width: u16, property_count: usize, comment_line_count: u16) {
        self.width = width;
        self.property_count = property_count;
        self.comment_line_count = comment_line_count;

        match &mut self.focused_position {
            None => {}
            Some(FocusedPosition::Property(index)) => {
                // NOTE: (現実的かはともかく)今回のupdateでpropertyリストが消えた場合は未実装
                if *index >= property_count {
                    *index = property_count.saturating_sub(1);
                }
            }
            Some(FocusedPosition::Comment(position)) => {
                if position.x >= width {
                    position.x = width.saturating_sub(1);
                }
                if position.y >= comment_line_count {
                    position.y = comment_line_count.saturating_sub(1);
                }
            }
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focused_position.as_ref()?;
        let action = Self::action_from_event(event)?;
        self.apply_action(action)
    }

    fn action_from_event(event: Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        let focused_position = self.focused_position.as_mut()?;
        match action {
            Action::MoveDown => {
                if let FocusedPosition::Property(index) = focused_position {
                    if *index + 1 < self.property_count {
                        *index += 1;
                    } else {
                        *focused_position = FocusedPosition::Comment(Position { x: 0, y: 0 });
                    }
                } else if let FocusedPosition::Comment(position) = focused_position {
                    if position.y + 1 < self.comment_line_count {
                        position.y += 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: position.x });
                    }
                }
            }
            Action::MoveUp => {
                if let FocusedPosition::Property(index) = focused_position {
                    if *index > 0 {
                        *index -= 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: 0 });
                    }
                } else if let FocusedPosition::Comment(position) = focused_position {
                    if position.y > 0 {
                        position.y -= 1;
                    } else if self.property_count > 0 {
                        *focused_position = FocusedPosition::Property(self.property_count - 1);
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: position.x });
                    }
                }
            }
            Action::MoveLeft => {
                if let FocusedPosition::Comment(position) = focused_position
                    && position.x > 0
                {
                    position.x -= 1;
                }
            }
            Action::MoveRight => {
                if let FocusedPosition::Comment(position) = focused_position
                    && position.x + 1 < self.width
                {
                    position.x += 1;
                }
            }
        }
        None
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::Focused { position } => {
                if self.property_count > 0 && position.y < self.property_count as u16 + 2 {
                    self.focused_position = Some(FocusedPosition::Property(min(
                        position.y.saturating_sub(2) as usize,
                        self.property_count.saturating_sub(1),
                    )));
                } else {
                    let comment_start_y = 2 + self.property_count as u16 + 1;
                    self.focused_position = Some(FocusedPosition::Comment(Position {
                        x: min(position.x, self.width.saturating_sub(1)),
                        y: min(
                            position.y.saturating_sub(comment_start_y),
                            self.comment_line_count.saturating_sub(1),
                        ),
                    }));
                }
            }
            FocusEvent::Unfocused => {
                self.focused_position = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                if self.property_count > 0 {
                    self.focused_position = Some(FocusedPosition::Property(0));
                } else {
                    self.focused_position = Some(FocusedPosition::Comment(Position { x, y: 0 }));
                }
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                self.focused_position = Some(FocusedPosition::Comment(Position {
                    x,
                    y: self.comment_line_count.saturating_sub(1),
                }));
            }
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        match self.focused_position {
            None => Position { x: 0, y: 0 },
            Some(FocusedPosition::Property(index)) => Position {
                x: 0,
                y: index as u16 + 2,
            },
            Some(FocusedPosition::Comment(position)) => Position {
                x: position.x,
                y: 2 + self.property_count as u16 + 1 + position.y,
            },
        }
    }

    pub fn is_focused(&self) -> bool {
        self.focused_position.is_some()
    }
}
