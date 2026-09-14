use crossterm::event::Event;
use ratatui::layout::Position;

use crate::stores::LocalJournalEntry;

use super::journals_list_item::focus_state;
use super::journals_list_item::focus_state::{FocusEvent, FocusState};
use super::journals_list_item::widget::local_state_marker;
use super::journals_list_item::{JournalItemWidget, JournalItemWidgetState, LocalJournalItemView};

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
}

/// Local Journalの表示内容、本文の描画cache、focus状態を保持するcomponent。
pub struct LocalJournalItemComponent {
    notes: String,
    state_marker: &'static str,
    comment_line_count: u16,
    focus_state: FocusState,
    widget_state: JournalItemWidgetState,
}

impl LocalJournalItemComponent {
    pub fn new() -> Self {
        Self {
            notes: String::new(),
            state_marker: "(local)",
            comment_line_count: 0,
            focus_state: FocusState::new(),
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn update(&mut self, entry: &LocalJournalEntry, width: u16) {
        self.notes.clone_from(&entry.journal.notes);
        self.state_marker = local_state_marker(&entry.state);
        self.widget_state
            .update(width, "", &chrono::Local::now(), &self.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
        self.focus_state
            .update(width, 0, self.comment_line_count, true);
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        match self.focus_state.process_event(event)? {
            focus_state::EventProcessResult::CursorLeavedFromBelow { x } => {
                Some(EventProcessResult::CursorLeavedFromBelow { x })
            }
            focus_state::EventProcessResult::CursorLeavedFromAbove { x } => {
                Some(EventProcessResult::CursorLeavedFromAbove { x })
            }
            focus_state::EventProcessResult::Edit
            | focus_state::EventProcessResult::SaveRequested => {
                // Local専用の編集経路が接続されるまでは、Remote用の編集・保存操作を伝播させない。
                None
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::entities::LocalJournal;
    use crate::stores::LocalJournalState;
    use crate::vos::IssueId;

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn process_event_edit_and_save_on_focused_local_item_are_no_ops() {
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

        assert!(
            component
                .process_event(key_event(KeyCode::Char('e'), KeyModifiers::NONE))
                .is_none()
        );
        assert!(
            component
                .process_event(key_event(KeyCode::Char('s'), KeyModifiers::CONTROL))
                .is_none()
        );
    }
}
