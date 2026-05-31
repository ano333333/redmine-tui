use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{BodyWidget, BodyWidgetState};
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;

use crate::app::Store;

pub struct BodyComponent {
    id: u16,
    focus_state: FocusState,
    widget_state: BodyWidgetState,
}

impl BodyComponent {
    pub fn new(id: u16, width: u16, height: u16) -> Self {
        Self {
            id,
            focus_state: FocusState::new(width, height),
            widget_state: BodyWidgetState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, store: &Store, width: u16) {
        if let Some(issue) = store.get_issue(self.id) {
            self.widget_state.update(width, &issue.body);
            let height = self.widget_state.line_count(width) as u16;
            self.focus_state.update(width, height);
        }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.widget_state.line_count(width) as u16
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let widget = BodyWidget::new(&self.widget_state);
        widget.render(area, buf);
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}
