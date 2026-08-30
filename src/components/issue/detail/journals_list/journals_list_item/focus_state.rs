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
    Edit,
}

enum FocusedPosition {
    Detail(usize),
    Notes(Position),
}

enum Action {
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    Edit,
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
            Some(FocusedPosition::Detail(index)) => {
                // NOTE: (現実的かはともかく)今回のupdateでpropertyリストが消えた場合は未実装
                if *index >= property_count {
                    *index = property_count.saturating_sub(1);
                }
            }
            Some(FocusedPosition::Notes(position)) => {
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
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    fn action_from_event(&self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            // 本文編集はNotes位置にフォーカスがある場合のみ許可する(Detail位置では無視)
            KeyCode::Char('e')
                if matches!(self.focused_position, Some(FocusedPosition::Notes(_))) =>
            {
                Some(Action::Edit)
            }
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        let focused_position = self.focused_position.as_mut()?;
        match action {
            Action::MoveDown => {
                if let FocusedPosition::Detail(index) = focused_position {
                    if *index + 1 < self.property_count {
                        *index += 1;
                    } else {
                        *focused_position = FocusedPosition::Notes(Position { x: 0, y: 0 });
                    }
                } else if let FocusedPosition::Notes(position) = focused_position {
                    if position.y + 1 < self.comment_line_count {
                        position.y += 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: position.x });
                    }
                }
            }
            Action::MoveUp => {
                if let FocusedPosition::Detail(index) = focused_position {
                    if *index > 0 {
                        *index -= 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: 0 });
                    }
                } else if let FocusedPosition::Notes(position) = focused_position {
                    if position.y > 0 {
                        position.y -= 1;
                    } else if self.property_count > 0 {
                        *focused_position = FocusedPosition::Detail(self.property_count - 1);
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: position.x });
                    }
                }
            }
            Action::MoveLeft => {
                if let FocusedPosition::Notes(position) = focused_position
                    && position.x > 0
                {
                    position.x -= 1;
                }
            }
            Action::MoveRight => {
                if let FocusedPosition::Notes(position) = focused_position
                    && position.x + 1 < self.width
                {
                    position.x += 1;
                }
            }
            // Notes位置の場合のみaction_from_eventで発生するが、防御的にDetail位置ではNoneを返す
            Action::Edit => {
                if matches!(focused_position, FocusedPosition::Notes(_)) {
                    return Some(EventProcessResult::Edit);
                }
            }
        }
        None
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::Focused { position } => {
                if self.property_count > 0 && position.y < self.property_count as u16 + 2 {
                    self.focused_position = Some(FocusedPosition::Detail(min(
                        position.y.saturating_sub(2) as usize,
                        self.property_count.saturating_sub(1),
                    )));
                } else {
                    let comment_start_y = 2 + self.property_count as u16 + 1;
                    self.focused_position = Some(FocusedPosition::Notes(Position {
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
                    self.focused_position = Some(FocusedPosition::Detail(0));
                } else {
                    self.focused_position = Some(FocusedPosition::Notes(Position { x, y: 0 }));
                }
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                self.focused_position = Some(FocusedPosition::Notes(Position {
                    x,
                    y: self.comment_line_count.saturating_sub(1),
                }));
            }
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        match self.focused_position {
            None => Position { x: 0, y: 0 },
            Some(FocusedPosition::Detail(index)) => Position {
                x: 0,
                y: index as u16 + 2,
            },
            Some(FocusedPosition::Notes(position)) => Position {
                x: position.x,
                y: 2 + self.property_count as u16 + 1 + position.y,
            },
        }
    }

    pub fn is_focused(&self) -> bool {
        self.focused_position.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    const WIDE_WIDTH: u16 = 32;
    const NARROW_WIDTH: u16 = 18;
    const NOTE_LINE_COUNT: u16 = 4;
    const NARROW_NOTE_LINE_COUNT: u16 = 5;
    const PLACEHOLDER_LINE_COUNT: u16 = 1;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn state(width: u16, property_count: usize, comment_line_count: u16) -> FocusState {
        let mut state = FocusState::new();
        state.update(width, property_count, comment_line_count);
        state
    }

    fn assert_leave_from_below(result: Option<EventProcessResult>, expected_x: u16) {
        match result {
            Some(EventProcessResult::CursorLeavedFromBelow { x }) => assert_eq!(x, expected_x),
            _ => panic!("expected cursor leave from below"),
        }
    }

    fn assert_leave_from_above(result: Option<EventProcessResult>, expected_x: u16) {
        match result {
            Some(EventProcessResult::CursorLeavedFromAbove { x }) => assert_eq!(x, expected_x),
            _ => panic!("expected cursor leave from above"),
        }
    }

    #[test]
    fn new_is_unfocused() {
        let state = FocusState::new();

        assert!(!state.is_focused());
        assert_eq!(state.get_cursor_position(), Position::new(0, 0));
    }

    #[test]
    fn update_clamps_comment_cursor_when_width_changes() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(31, 6),
        });

        state.update(NARROW_WIDTH, 1, NARROW_NOTE_LINE_COUNT);

        assert_eq!(state.get_cursor_position(), Position::new(17, 6));
    }

    #[test]
    fn update_clamps_property_focus_to_last_property() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(0, 3),
        });

        state.update(WIDE_WIDTH, 1, NOTE_LINE_COUNT);

        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn update_removed_notes_keeps_comment_focus_on_placeholder_line() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        state.update(WIDE_WIDTH, 2, PLACEHOLDER_LINE_COUNT);

        assert_eq!(state.get_cursor_position(), Position::new(6, 5));
    }

    #[test]
    fn focus_from_above_focuses_first_property_when_properties_exist() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        assert!(state.is_focused());
        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn focus_from_above_without_properties_focuses_first_comment_line() {
        let mut state = state(WIDE_WIDTH, 0, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        assert_eq!(state.get_cursor_position(), Position::new(6, 3));
    }

    #[test]
    fn focus_from_below_focuses_last_comment_line() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        assert_eq!(state.get_cursor_position(), Position::new(6, 8));
    }

    #[test]
    fn focus_from_below_with_empty_notes_focuses_placeholder_line() {
        let mut state = state(WIDE_WIDTH, 2, PLACEHOLDER_LINE_COUNT);

        state.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        assert_eq!(state.get_cursor_position(), Position::new(6, 5));
    }

    #[test]
    fn focused_position_above_properties_focuses_first_property() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 0),
        });

        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn focused_position_past_properties_with_empty_notes_focuses_placeholder_line() {
        let mut state = state(WIDE_WIDTH, 2, PLACEHOLDER_LINE_COUNT);

        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 99),
        });

        assert_eq!(state.get_cursor_position(), Position::new(6, 5));
    }

    #[test]
    fn focused_position_near_top_without_properties_focuses_first_comment_line() {
        let mut state = state(WIDE_WIDTH, 0, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 0),
        });

        assert_eq!(state.get_cursor_position(), Position::new(6, 3));
    }

    #[test]
    fn focused_position_past_comment_with_properties_focuses_last_comment_line() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 99),
        });

        assert_eq!(state.get_cursor_position(), Position::new(6, 8));
    }

    #[test]
    fn focused_position_past_comment_without_properties_focuses_last_comment_line() {
        let mut state = state(WIDE_WIDTH, 0, NOTE_LINE_COUNT);

        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 99),
        });

        assert_eq!(state.get_cursor_position(), Position::new(6, 6));
    }

    #[test]
    fn unfocused_clears_focus() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        state.focus_event(FocusEvent::Unfocused);

        assert!(!state.is_focused());
        assert_eq!(state.get_cursor_position(), Position::new(0, 0));
    }

    #[test]
    fn process_event_j_moves_between_properties() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 3));
    }

    #[test]
    fn process_event_j_on_last_property_with_empty_notes_moves_to_placeholder_line() {
        let mut state = state(WIDE_WIDTH, 1, PLACEHOLDER_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 4));
    }

    #[test]
    fn process_event_j_moves_from_last_property_to_first_comment_line() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 4));
    }

    #[test]
    fn process_event_j_moves_between_comment_lines() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });
        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 5));
    }

    #[test]
    fn process_event_j_on_last_comment_line_returns_leave_from_below() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert_leave_from_below(result, 6);
        assert_eq!(state.get_cursor_position(), Position::new(6, 7));
    }

    #[test]
    fn process_event_k_moves_between_properties() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn process_event_k_moves_from_first_comment_line_to_last_property() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 4),
        });

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 3));
    }

    #[test]
    fn process_event_k_moves_between_comment_lines() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 5),
        });

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(6, 4));
    }

    #[test]
    fn process_event_k_on_first_comment_line_without_properties_returns_leave_from_above() {
        let mut state = state(WIDE_WIDTH, 0, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert_leave_from_above(result, 6);
        assert_eq!(state.get_cursor_position(), Position::new(6, 3));
    }

    #[test]
    fn process_event_k_on_first_property_returns_leave_from_above() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert_leave_from_above(result, 0);
        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn process_event_h_and_l_move_comment_x_and_clamp_at_edges() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(1, 3),
        });

        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 4));
        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 4));

        for _ in 0..40 {
            assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        }

        assert_eq!(state.get_cursor_position(), Position::new(31, 4));
    }

    #[test]
    fn process_event_when_unfocused_does_nothing() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 0));
    }

    #[test]
    fn process_event_unmapped_key_keeps_focus_position() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('x')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }

    #[test]
    fn process_event_e_on_notes_position_returns_edit() {
        let mut state = state(WIDE_WIDTH, 1, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::Focused {
            position: Position::new(6, 4),
        });

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(result, Some(EventProcessResult::Edit)));
        assert_eq!(state.get_cursor_position(), Position::new(6, 4));
    }

    #[test]
    fn process_event_e_on_detail_position_does_nothing() {
        let mut state = state(WIDE_WIDTH, 2, NOTE_LINE_COUNT);
        state.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = state.process_event(key_event(KeyCode::Char('e')));

        assert!(result.is_none());
        assert_eq!(state.get_cursor_position(), Position::new(0, 2));
    }
}
