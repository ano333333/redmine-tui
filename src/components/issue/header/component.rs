use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::HeaderWidget;
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;

use crate::app::Store;

pub struct HeaderComponent {
    id: u16,
    focus_state: FocusState,
}

impl HeaderComponent {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            focus_state: FocusState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self) {}

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some(issue) = store.get_issue(self.id) {
            let widget = HeaderWidget {
                id: self.id,
                title: &issue.title,
                creator: &issue.creator,
                appended_at: issue.appended_at,
                updated_at: issue.updated_at,
            };
            widget.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn render(&self, store: &Store, area: Rect, buf: &mut Buffer) {
        if let Some(issue) = store.get_issue(self.id) {
            let widget = HeaderWidget {
                id: self.id,
                title: &issue.title,
                creator: &issue.creator,
                appended_at: issue.appended_at,
                updated_at: issue.updated_at,
            };
            widget.render(area, buf);
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}
