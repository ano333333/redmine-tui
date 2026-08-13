use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::HeaderWidget;
use crossterm::event::Event;
use ratatui::layout::Position;

use crate::app::{IssueState, Store};
use crate::vos::IssueId;

pub struct HeaderComponent {
    id: IssueId,
    focus_state: FocusState,
}

impl HeaderComponent {
    pub fn new(id: impl Into<IssueId>) -> Self {
        Self {
            id: id.into(),
            focus_state: FocusState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some((issue, issue_status)) = store.get_issue(self.id) {
            // FIXME: Storeのsynced/editedをwidgetに反映
            let widget = HeaderWidget::new(
                self.id,
                &issue.subject,
                self.focus_state.is_focused(),
                issue_status == IssueState::Synced,
            );
            widget.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> HeaderWidget<'a> {
        let (issue, issue_status) = store
            .get_issue(self.id)
            .expect("HeaderComponent requires its issue to exist in Store");
        HeaderWidget::new(
            self.id,
            &issue.subject,
            self.focus_state.is_focused(),
            issue_status == IssueState::Synced,
        )
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}
