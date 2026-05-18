use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::PropertyWidget;
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;

use crate::app::Store;
use crate::entities::Issue;

pub struct PropertyComponent {
    id: u16,
    focus_state: FocusState,
}

impl PropertyComponent {
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

    pub fn update(&mut self, _: &Store) {}

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some(issue) = store.get_issue(self.id) {
            let paragraph = create_property_widget(issue);
            paragraph.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn render(&self, store: &Store, area: Rect, buf: &mut Buffer) {
        if let Some(issue) = store.get_issue(self.id) {
            let widget = create_property_widget(&issue);
            widget.render(area, buf);
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

fn create_property_widget<'a>(issue: &'a Issue) -> PropertyWidget<'a> {
    PropertyWidget {
        id: issue.id,
        status: &issue.status,
        priority: &issue.priority,
        person_in_charge: &issue.person_in_charge,
        target_version: &issue.target_version,
        start_date: issue.start_date,
        due: issue.due,
        progress: issue.progress,
        planned_hours: issue.planned_hours,
        resolve_way: &issue.resolve_way,
        component: &issue.component,
        tags: &issue.tags,
    }
}
