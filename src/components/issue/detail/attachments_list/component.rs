use ratatui::layout::Position;

use crate::platform::input::InputEvent;
use crate::stores::Store;
use crate::vos::IssueId;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{AttachmentRow, AttachmentsListWidget, line_count};

pub struct AttachmentsListComponent {
    id: IssueId,
    focus_state: FocusState,
}

impl AttachmentsListComponent {
    pub fn new(id: impl Into<IssueId>) -> Self {
        Self {
            id: id.into(),
            focus_state: FocusState::new(),
        }
    }

    pub fn update(&mut self, store: &Store) {
        // 末尾の追加行の分を足す。
        self.focus_state
            .update(store.get_issue(self.id).0.attachments().len() + 1);
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> AttachmentsListWidget<'a> {
        let rows = store
            .get_issue(self.id)
            .0
            .attachments()
            .iter()
            .map(|attachment| AttachmentRow {
                filename: &attachment.filename,
                filesize: attachment.filesize,
                author: &attachment.author,
                created_on: attachment.created_on,
            })
            .collect();
        AttachmentsListWidget::new(rows, self.focus_state.focused_index())
    }

    pub fn line_count(&self, store: &Store) -> u16 {
        line_count(store.get_issue(self.id).0.attachments().len())
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn process_event(&mut self, event: &InputEvent) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}
