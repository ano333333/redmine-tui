use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::PropertyWidget;
use crossterm::event::Event;
use ratatui::layout::Position;

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
        if let Some((issue, _)) = store.get_issue(self.id) {
            let paragraph = create_property_widget(issue, None);
            paragraph.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> PropertyWidget<'a> {
        let (issue, _) = store.get_issue(self.id).unwrap();
        create_property_widget(issue, self.focus_state.focused_y())
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

fn create_property_widget<'a>(issue: &'a Issue, focused_y: Option<u16>) -> PropertyWidget<'a> {
    PropertyWidget::new(
        issue.id,
        &issue.status,
        &issue.priority,
        &issue.person_in_charge,
        &issue.target_version,
        issue.start_date,
        issue.due,
        issue.progress,
        issue.planned_hours,
        &issue.resolve_way,
        &issue.component,
        &issue.tags,
        focused_y,
    )
}
