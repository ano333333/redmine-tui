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
    OpenSpentTimeInputPopup,
}

enum Action {
    MoveDown,
    MoveUp,
    OpenIssueStatusPopup,
    OpenSpentTimeInputPopup,
}

pub struct FocusState {
    focused_y: Option<u16>,
}

impl FocusState {
    pub fn new() -> Self {
        Self { focused_y: None }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
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

    fn action_from_event(&self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };

        let focused_y = self.focused_y?;

        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('e') if focused_y == 3 => Some(Action::OpenIssueStatusPopup),
            KeyCode::Char('a') if focused_y == LINE_COUNT - 1 => {
                Some(Action::OpenSpentTimeInputPopup)
            }
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::MoveDown => {
                let focused_y = self.focused_y.as_mut()?;
                if *focused_y + 1 == LINE_COUNT {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                *focused_y += 1;
                None
            }
            Action::MoveUp => {
                let focused_y = self.focused_y.as_mut()?;
                if *focused_y == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                *focused_y -= 1;
                None
            }
            Action::OpenIssueStatusPopup => Some(EventProcessResult::OpenIssueStatusPopup),
            Action::OpenSpentTimeInputPopup => Some(EventProcessResult::OpenSpentTimeInputPopup),
        }
    }
}

