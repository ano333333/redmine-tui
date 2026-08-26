use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::Journal;
use crate::vos::{EntityIdValue, JournalId};

use super::focus_state;
use super::focus_state::FocusState;
pub use super::focus_state::FocusEvent;
use super::{JournalItemWidget, JournalItemWidgetState};

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
    EditRequested { id: JournalId, notes: String },
}

pub struct JournalsListItemComponent {
    pub id: u16,
    journal: Journal,
    comment_line_count: u16,
    focus_state: FocusState,
    widget_state: JournalItemWidgetState,
}

impl JournalsListItemComponent {
    pub fn new(journal: &Journal) -> Self {
        Self {
            id: journal.id.get(),
            journal: journal.clone(),
            comment_line_count: 0,
            focus_state: FocusState::new(),
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .map(|result| match result {
                focus_state::EventProcessResult::CursorLeavedFromBelow { x } => {
                    EventProcessResult::CursorLeavedFromBelow { x }
                }
                focus_state::EventProcessResult::CursorLeavedFromAbove { x } => {
                    EventProcessResult::CursorLeavedFromAbove { x }
                }
                focus_state::EventProcessResult::Edit => EventProcessResult::EditRequested {
                    id: self.journal.id,
                    notes: self.journal.notes.clone(),
                },
            })
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, journal: &Journal, width: u16) {
        self.journal = journal.clone();
        self.widget_state
            .update(width, &journal.user, &journal.updated_on, &journal.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
        self.focus_state
            .update(width, self.journal.details.len(), self.comment_line_count);
    }

    pub fn create_widget<'a>(&'a self) -> JournalItemWidget<'a> {
        JournalItemWidget::new(
            &self.journal,
            &self.widget_state,
            self.focus_state.is_focused(),
        )
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.journal.details.len() as u16 + 1 + self.comment_line_count
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        entities::Journal,
        test_support::{local_datetime, render_snapshot},
        vos::{JournalDetail, JournalDetailAttr, JournalId},
    };

    const WIDE_WIDTH: u16 = 32;
    const NARROW_WIDTH: u16 = 18;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn create_journal(id: u16, details: Vec<JournalDetail>, notes: impl Into<String>) -> Journal {
        Journal {
            id: JournalId::new(id),
            user: "alice".to_string(),
            updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
            details,
            notes: notes.into(),
        }
    }

    fn assigned_to_detail(old: Option<&str>, new: Option<&str>) -> JournalDetail {
        JournalDetail::Attr(JournalDetailAttr::AssignedTo {
            old: old.map(str::to_string),
            new: new.map(str::to_string),
        })
    }

    fn status_detail(old: &str, new: &str) -> JournalDetail {
        JournalDetail::Attr(JournalDetailAttr::StatusId {
            old: old.to_string(),
            new: new.to_string(),
        })
    }

    fn details() -> Vec<JournalDetail> {
        vec![
            status_detail("新規", "進行中"),
            assigned_to_detail(None, Some("bob")),
        ]
    }

    fn one_detail() -> Vec<JournalDetail> {
        vec![assigned_to_detail(None, Some("bob"))]
    }

    fn notes() -> &'static str {
        "first paragraph\n\nsecond paragraph with wrapping words"
    }

    fn updated_notes() -> &'static str {
        "updated notes with enough text to wrap onto a different set of rendered lines"
    }

    fn component_with_update(journal: &Journal, width: u16) -> JournalsListItemComponent {
        let mut component = JournalsListItemComponent::new(journal);
        component.update(journal, width);
        component
    }

    fn assert_layout_contract(
        component: &JournalsListItemComponent,
        width: u16,
        line_count: u16,
        cursor: Position,
    ) {
        assert_eq!(component.line_count(width), line_count);
        assert_eq!(component.get_cursor_position(), cursor);
    }

    #[test]
    fn update_initial_state_is_unfocused_and_rendered() {
        let journal = create_journal(1, details(), notes());
        let mut component = JournalsListItemComponent::new(&journal);

        component.update(&journal, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_initial_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn update_changed_notes_updates_line_count_and_widget() {
        let initial_journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        let updated_journal = create_journal(1, one_detail(), updated_notes());

        component.update(&updated_journal, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 7, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_update_changed_notes",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn focus_event_updates_cursor_and_widget_focus() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 2));
        render_snapshot(
            "journals_list_item_component_focus_from_above_to_detail",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn unfocused_removes_widget_focus() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        component.focus_event(FocusEvent::Unfocused);

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn process_event_delegates_to_focus_state() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 3));
    }

    #[test]
    fn process_event_e_on_notes_position_returns_edit_requested_with_current_notes() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested { id, notes: edit_notes }) => {
                assert_eq!(id, JournalId::new(1));
                assert_eq!(edit_notes, notes());
            }
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    fn update_passes_latest_width_and_comment_line_count_to_focus_state() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(31, 6),
        });

        component.update(&journal, NARROW_WIDTH);

        assert_layout_contract(&component, NARROW_WIDTH, 9, Position::new(17, 6));
    }

    #[test]
    fn update_passes_latest_property_count_to_focus_state() {
        let initial_journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(0, 3),
        });
        let updated_journal = create_journal(1, one_detail(), notes());

        component.update(&updated_journal, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 8, Position::new(0, 2));
    }
}
