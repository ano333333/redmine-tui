use ratatui::layout::Position;

use crate::platform::input::InputEvent;
use crate::stores::{IssueState, Store};
use crate::vos::IssueId;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{AttachmentRow, AttachmentsListWidget, UploadingRow, row_count};

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
        self.focus_state.update(self.row_count(store));
    }

    /// Issue属性・Journal・添付ファイルの保存中は、Storeが添付ファイルの追加を受け付けない。
    pub fn can_add(&self, store: &Store) -> bool {
        store.try_get_issue_state(self.id) != Some(IssueState::Uploading)
            && !store.has_uploading_journal(self.id)
            && store.try_get_attachment_upload(self.id).is_none()
    }

    fn row_count(&self, store: &Store) -> usize {
        row_count(
            store.get_issue(self.id).0.attachments().len(),
            store.try_get_attachment_upload(self.id).is_some(),
        )
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
        let uploading = store
            .try_get_attachment_upload(self.id)
            .map(|upload| UploadingRow {
                filename: &upload.filename,
                stage: upload.stage,
            });
        AttachmentsListWidget::new(
            rows,
            uploading,
            self.can_add(store),
            self.focus_state.focused_index(),
        )
    }

    pub fn line_count(&self, store: &Store) -> u16 {
        self.create_widget(store).line_count()
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
