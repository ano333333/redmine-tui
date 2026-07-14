use super::focus_state;
use super::focus_state::FocusState;
use super::widget::{BodyWidget, BodyWidgetState};
use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::Issue;

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
    EditRequested { id: u16, body: String },
}

pub struct BodyComponent {
    id: u16,
    focus_state: FocusState,
    body: String,
    widget_state: BodyWidgetState,
}

impl BodyComponent {
    pub fn new(id: u16, width: u16, height: u16) -> Self {
        Self {
            id,
            focus_state: FocusState::new(width, height),
            body: String::new(),
            widget_state: BodyWidgetState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .map(|result| match result {
                focus_state::EventProcessResult::CursorLeavedFromBelow { x } => {
                    EventProcessResult::CursorLeavedFromBelow { x }
                }
                focus_state::EventProcessResult::CursorLeavedFromAbove { x } => {
                    EventProcessResult::CursorLeavedFromAbove { x }
                }
                focus_state::EventProcessResult::Edit => EventProcessResult::EditRequested {
                    id: self.id,
                    body: self.body.clone(),
                },
            })
    }

    pub fn focus_event(&mut self, event: super::FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, issue: &Issue, width: u16) {
        self.body = issue.description.clone();
        self.widget_state.update(width, &self.body);
        let height = self.widget_state.line_count(width) as u16;
        self.focus_state.update(width, height);
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.widget_state.line_count(width) as u16
    }

    pub fn create_widget<'a>(&'a self) -> BodyWidget<'a> {
        BodyWidget::new(&self.widget_state, self.focus_state.is_focused())
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}
