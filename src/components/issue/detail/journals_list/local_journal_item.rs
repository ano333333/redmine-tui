use ratatui::layout::Position;

use crate::inputs::InputEvent;
use crate::stores::{LocalJournalEntry, LocalJournalState};

use super::journals_list_item::focus_state;
use super::journals_list_item::focus_state::{FocusEvent, FocusState};
use super::journals_list_item::widget::local_state_marker;
use super::journals_list_item::{JournalItemWidget, JournalItemWidgetState, LocalJournalItemView};

pub enum EventProcessResult {
    CursorLeavedFromBelow {
        x: u16,
    },
    CursorLeavedFromAbove {
        x: u16,
    },
    EditRequested {
        notes: String,
    },
    /// 編集キーを正常なno-opとして消費済みであり、未処理を表す`None`とは区別する。
    EditSuppressed,
    SaveRequested,
    /// 保存キーを正常なno-opとして消費済みであり、未処理を表す`None`とは区別する。
    SaveSuppressed,
}

/// Local Journalの表示内容、本文の描画cache、focus状態を保持するcomponent。
pub struct LocalJournalItemComponent {
    notes: String,
    state_marker: &'static str,
    comment_line_count: u16,
    editable: bool,
    focus_state: FocusState,
    widget_state: JournalItemWidgetState,
}

impl LocalJournalItemComponent {
    pub fn new() -> Self {
        Self {
            notes: String::new(),
            state_marker: "(local)",
            comment_line_count: 0,
            editable: false,
            focus_state: FocusState::new(),
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn update(&mut self, entry: &LocalJournalEntry, width: u16) {
        self.notes.clone_from(&entry.journal.notes);
        self.state_marker = local_state_marker(&entry.state);
        self.widget_state
            .update(width, "", Some(&chrono::Local::now()), &self.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
        self.editable = matches!(entry.state, LocalJournalState::LocalOnly { .. });
        self.focus_state
            .update(width, 0, self.comment_line_count, !self.editable);
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        match self.focus_state.process_event(event)? {
            focus_state::EventProcessResult::CursorLeavedFromBelow { x } => {
                Some(EventProcessResult::CursorLeavedFromBelow { x })
            }
            focus_state::EventProcessResult::CursorLeavedFromAbove { x } => {
                Some(EventProcessResult::CursorLeavedFromAbove { x })
            }
            focus_state::EventProcessResult::Edit if self.editable => {
                Some(EventProcessResult::EditRequested {
                    notes: self.notes.clone(),
                })
            }
            focus_state::EventProcessResult::SaveRequested if self.editable => {
                Some(EventProcessResult::SaveRequested)
            }
            focus_state::EventProcessResult::SaveSuppressed => {
                Some(EventProcessResult::SaveSuppressed)
            }
            focus_state::EventProcessResult::Edit => Some(EventProcessResult::EditSuppressed),
            focus_state::EventProcessResult::SaveRequested => {
                Some(EventProcessResult::SaveSuppressed)
            }
        }
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn create_widget(&self, focused: bool) -> JournalItemWidget<'_> {
        JournalItemWidget::new(
            LocalJournalItemView {
                notes: &self.notes,
                state_marker: self.state_marker,
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
    use crate::inputs::{InputEvent, KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::entities::LocalJournal;
    use crate::stores::LocalJournalState;
    use crate::vos::IssueId;

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn process_event_e_on_focused_local_item_returns_edit_requested() {
        let entry = LocalJournalEntry {
            journal: LocalJournal {
                issue_id: IssueId::new(1),
                notes: "local notes".to_string(),
            },
            state: LocalJournalState::LocalOnly { failure: None },
        };
        let mut component = LocalJournalItemComponent::new();
        component.update(&entry, 32);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        match component.process_event(key_event(KeyCode::Char('e'), KeyModifiers::none())) {
            Some(EventProcessResult::EditRequested { notes }) => assert_eq!(notes, "local notes"),
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    fn process_event_ctrl_s_on_focused_local_only_item_returns_save_requested() {
        let entry = LocalJournalEntry {
            journal: LocalJournal {
                issue_id: IssueId::new(1),
                notes: "local notes".to_string(),
            },
            state: LocalJournalState::LocalOnly { failure: None },
        };
        let mut component = LocalJournalItemComponent::new();
        component.update(&entry, 32);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('s'), KeyModifiers::control())),
            Some(EventProcessResult::SaveRequested)
        ));
    }

    #[test]
    fn process_event_e_and_ctrl_s_on_uploading_local_item_are_suppressed() {
        let entry = LocalJournalEntry {
            journal: LocalJournal {
                issue_id: IssueId::new(1),
                notes: "local notes".to_string(),
            },
            state: LocalJournalState::Uploading,
        };
        let mut component = LocalJournalItemComponent::new();
        component.update(&entry, 32);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('e'), KeyModifiers::none())),
            Some(EventProcessResult::EditSuppressed)
        ));
        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('s'), KeyModifiers::control())),
            Some(EventProcessResult::SaveSuppressed)
        ));
    }
}
