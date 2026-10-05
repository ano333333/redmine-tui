use ratatui::layout::Position;

use crate::platform::input::InputEvent;
use crate::stores::{DeletedJournalEntry, DeletedJournalState};
use crate::vos::JournalId;

use super::journals_list_item::focus_state;
use super::journals_list_item::focus_state::{FocusEvent, FocusState};
use super::journals_list_item::{JournalItemWidget, JournalItemWidgetState, LocalJournalItemView};

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
    EditRequested { notes: String },
    SaveRequested,
    DiscardRequested,
    Handled,
}

/// 取得結果から消えて退避したJournalの表示内容、本文の描画cache、focus状態を保持するcomponent。
///
/// 投稿前の手元の本文として、Local Journalと同じ表示と操作に破棄を加えて扱う。
pub struct DeletedJournalItemComponent {
    pub original_id: JournalId,
    notes: String,
    state_marker: String,
    comment_line_count: u16,
    pending: bool,
    focus_state: FocusState,
    widget_state: JournalItemWidgetState,
}

impl DeletedJournalItemComponent {
    pub fn new(original_id: JournalId) -> Self {
        Self {
            original_id,
            notes: String::new(),
            state_marker: String::new(),
            comment_line_count: 0,
            pending: false,
            focus_state: FocusState::new(),
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn update(&mut self, entry: &DeletedJournalEntry, width: u16) {
        self.notes.clone_from(&entry.notes);
        self.pending = matches!(entry.state, DeletedJournalState::Pending { .. });
        self.state_marker = if self.pending {
            format!("(deleted #{})", entry.original_id)
        } else {
            format!("(deleted #{}) (uploading)", entry.original_id)
        };
        self.widget_state
            .update(width, "", Some(&chrono::Local::now()), &self.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
        self.focus_state
            .update(width, 0, self.comment_line_count, !self.pending);
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        Some(match self.focus_state.process_event(event)? {
            focus_state::EventProcessResult::CursorLeavedFromBelow { x } => {
                EventProcessResult::CursorLeavedFromBelow { x }
            }
            focus_state::EventProcessResult::CursorLeavedFromAbove { x } => {
                EventProcessResult::CursorLeavedFromAbove { x }
            }
            focus_state::EventProcessResult::Edit if self.pending => {
                EventProcessResult::EditRequested {
                    notes: self.notes.clone(),
                }
            }
            focus_state::EventProcessResult::SaveRequested if self.pending => {
                EventProcessResult::SaveRequested
            }
            focus_state::EventProcessResult::DiscardRequested if self.pending => {
                EventProcessResult::DiscardRequested
            }
            // 投稿中は編集・保存・破棄のキーを受け取っても何もしない。
            focus_state::EventProcessResult::Edit
            | focus_state::EventProcessResult::SaveRequested
            | focus_state::EventProcessResult::DiscardRequested
            | focus_state::EventProcessResult::Handled => EventProcessResult::Handled,
        })
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn create_widget(&self, focused: bool) -> JournalItemWidget<'_> {
        JournalItemWidget::new(
            LocalJournalItemView {
                notes: &self.notes,
                state_marker: &self.state_marker,
            },
            vec![],
            &self.widget_state,
            focused,
        )
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + 1 + self.comment_line_count + 1
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

#[cfg(test)]
mod tests {
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    const WIDTH: u16 = 40;

    fn entry(state: DeletedJournalState) -> DeletedJournalEntry {
        DeletedJournalEntry {
            original_id: JournalId::new(3),
            notes: "deleted notes".to_string(),
            state,
        }
    }

    fn focused_component(state: DeletedJournalState) -> DeletedJournalItemComponent {
        let mut component = DeletedJournalItemComponent::new(JournalId::new(3));
        component.update(&entry(state), WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        component
    }

    fn key(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn ctrl_s() -> InputEvent {
        InputEvent::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::control()))
    }

    #[test]
    fn a_pending_item_requests_edit_save_and_discard() {
        let mut component = focused_component(DeletedJournalState::Pending { failure: None });

        assert!(matches!(
            component.process_event(key(KeyCode::Char('e'))),
            Some(EventProcessResult::EditRequested { notes }) if notes == "deleted notes"
        ));
        assert!(matches!(
            component.process_event(ctrl_s()),
            Some(EventProcessResult::SaveRequested)
        ));
        assert!(matches!(
            component.process_event(key(KeyCode::Char('d'))),
            Some(EventProcessResult::DiscardRequested)
        ));
    }

    #[test]
    fn an_uploading_item_handles_edit_save_and_discard_without_requests() {
        let mut component = focused_component(DeletedJournalState::Uploading);

        assert!(matches!(
            component.process_event(key(KeyCode::Char('e'))),
            Some(EventProcessResult::Handled)
        ));
        assert!(matches!(
            component.process_event(ctrl_s()),
            Some(EventProcessResult::Handled)
        ));
        assert!(matches!(
            component.process_event(key(KeyCode::Char('d'))),
            Some(EventProcessResult::Handled)
        ));
    }

    #[test]
    fn the_marker_shows_the_original_id_and_the_upload_state() {
        let mut component = DeletedJournalItemComponent::new(JournalId::new(3));

        component.update(
            &entry(DeletedJournalState::Pending { failure: None }),
            WIDTH,
        );
        assert_eq!(component.state_marker, "(deleted #3)");
        component.update(&entry(DeletedJournalState::Uploading), WIDTH);
        assert_eq!(component.state_marker, "(deleted #3) (uploading)");
    }
}
