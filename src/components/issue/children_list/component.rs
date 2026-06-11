use crossterm::event::Event;
use ratatui::layout::Position;

use crate::app::Store;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::ChildrenListWidget;

pub struct ChildrenListComponent {
    id: u16,
    focus_state: FocusState,
}

impl ChildrenListComponent {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            focus_state: FocusState::new(),
        }
    }

    pub fn update(&mut self, store: &Store) {
        if let Some((issue, _)) = store.get_issue(self.id) {
            self.focus_state.update(&issue.child_ids);
        }
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> ChildrenListWidget<'a> {
        let (issue, _) = store.get_issue(self.id).unwrap();

        let child_all_num = issue.child_ids.len() as u16;
        let child_complete_num = issue
            .child_ids
            .iter()
            .map(|id| store.get_issue(*id))
            .filter(|issue| issue.is_some_and(|(issue, _)| issue.is_completed()))
            .count() as u16;
        let child_incomplete_num = child_all_num - child_complete_num;

        let children: Vec<_> = issue
            .child_ids
            .iter()
            .filter_map(|id| store.get_issue(*id))
            .map(|(issue, _)| issue)
            .collect();

        ChildrenListWidget::new(
            child_all_num,
            child_complete_num,
            child_incomplete_num,
            children,
            self.focus_state.focused_index(),
        )
    }

    pub fn line_count(&self, store: &Store) -> u16 {
        if let Some((issue, _)) = store.get_issue(self.id) {
            2 + (issue.child_ids.len() as u16) + 1
        } else {
            0
        }
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn process_event(&mut self, event: &Event) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}
