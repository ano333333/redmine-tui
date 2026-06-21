use crossterm::event::Event;
use ratatui::layout::Position;

use crate::app::Store;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{ChildIssueRow, ChildrenListWidget};

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
        let child_closed_num = issue
            .child_ids
            .iter()
            .filter(|id| {
                store.get_issue(**id).is_some_and(|(issue, _)| {
                    let issue_status = store.get_issue_status(issue.status_id);
                    issue_status.is_closed
                })
            })
            .count() as u16;
        let child_opened_num = child_all_num - child_closed_num;

        let children: Vec<_> = issue
            .child_ids
            .iter()
            .filter_map(|id| store.get_issue(*id))
            .map(|(issue, _)| ChildIssueRow {
                issue,
                issue_status: store.get_issue_status(issue.status_id),
                assigned_to_name: issue
                    .assigned_to_id
                    .and_then(|user_id| store.get_user(user_id))
                    .map(|user| user.name.as_str()),
            })
            .collect();

        ChildrenListWidget::new(
            child_all_num,
            child_closed_num,
            child_opened_num,
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
