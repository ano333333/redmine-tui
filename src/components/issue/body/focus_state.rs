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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn process_event_ignores_key_when_unfocused() {
        let mut state = FocusState::new(10, 5);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 0));
        assert!(!state.is_focused());
    }

    #[test]
    fn process_event_h_moves_cursor_left() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(3, 2),
        });

        let result = state.process_event(key_event(KeyCode::Char('h')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(2, 2));
    }

    #[test]
    fn process_event_l_moves_cursor_right_within_width() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(3, 2),
        });

        let result = state.process_event(key_event(KeyCode::Char('l')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(4, 2));
    }

    #[test]
    fn process_event_j_moves_cursor_down() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(3, 2),
        });

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(3, 3));
    }

    #[test]
    fn process_event_k_moves_cursor_up() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(3, 2),
        });

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(3, 1));
    }

    #[test]
    fn process_event_j_on_last_line_returns_leave_from_below() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::CursorEnteredFromBelow { x: 4 });

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow { x: 4 })
        ));
        assert_eq!(state.get_cursor_position(), Position::new(4, 4));
    }

    #[test]
    fn process_event_k_on_first_line_returns_leave_from_above() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 4 });

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromAbove { x: 4 })
        ));
        assert_eq!(state.get_cursor_position(), Position::new(4, 0));
    }

    #[test]
    fn process_event_e_returns_edit() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(3, 2),
        });

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(result, Some(EventProcessResult::Edit)));
        assert_eq!(state.get_cursor_position(), Position::new(3, 2));
    }

    #[test]
    fn focus_event_clamps_position_to_bounds() {
        let mut state = FocusState::new(10, 5);

        state.focus_event(FocusEvent::Focused {
            position: Position::new(20, 9),
        });

        assert_eq!(state.get_cursor_position(), Position::new(9, 4));
        assert!(state.is_focused());
    }

    #[test]
    fn update_clamps_existing_cursor_to_new_size() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(8, 4),
        });

        state.update(4, 3);

        assert_eq!(state.get_cursor_position(), Position::new(4, 3));
    }

    #[test]
    fn focus_event_unfocused_clears_focus() {
        let mut state = FocusState::new(10, 5);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(3, 2),
        });

        state.focus_event(FocusEvent::Unfocused);

        assert_eq!(state.get_cursor_position(), Position::new(0, 0));
        assert!(!state.is_focused());
    }
}
