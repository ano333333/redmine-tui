use std::cmp::min;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

use crate::entities::{EntityIdValue, Journal};

use super::{JournalItemWidget, JournalItemWidgetState};

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove { x: u16 },
    CursorEnteredFromBelow { x: u16 },
}

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
}

enum FocusedPosition {
    Property(usize),
    Comment(Position),
}

pub struct JournalsListItemComponent {
    pub id: u16,
    width: u16,
    journal: Journal,
    comment_line_count: u16,
    focused_position: Option<FocusedPosition>,
    widget_state: JournalItemWidgetState,
}

impl JournalsListItemComponent {
    pub fn new(journal: &Journal) -> Self {
        Self {
            id: journal.id.get(),
            width: 0,
            journal: journal.clone(),
            comment_line_count: 0,
            focused_position: None,
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        if self.focused_position.is_none() {
            return None;
        }
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Char('j') => {
                if let Some(FocusedPosition::Property(index)) = &mut self.focused_position {
                    if *index + 1 < self.journal.details.len() {
                        *index += 1;
                    } else {
                        self.focused_position =
                            Some(FocusedPosition::Comment(Position { x: 0, y: 0 }));
                    }
                } else if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                {
                    if position.y + 1 < self.comment_line_count {
                        position.y += 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: position.x });
                    }
                }
            }
            KeyCode::Char('k') => {
                if let Some(FocusedPosition::Property(index)) = &mut self.focused_position {
                    if *index > 0 {
                        *index -= 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: 0 });
                    }
                } else if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                {
                    if position.y > 0 {
                        position.y -= 1;
                    } else if self.journal.details.len() > 0 {
                        self.focused_position =
                            Some(FocusedPosition::Property(self.journal.details.len() - 1));
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: position.x });
                    }
                }
            }
            KeyCode::Char('h') => {
                if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                    && position.x > 0
                {
                    position.x -= 1;
                }
            }
            KeyCode::Char('l') => {
                if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                    && position.x + 1 < self.width
                {
                    position.x += 1;
                }
            }
            _ => {}
        }
        None
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        let property_count = self.journal.details.len();
        match event {
            FocusEvent::Focused { position } => {
                if property_count > 0 && position.y < property_count as u16 + 2 {
                    self.focused_position = Some(FocusedPosition::Property(min(
                        position.y.saturating_sub(2) as usize,
                        property_count.saturating_sub(1),
                    )));
                } else {
                    let comment_start_y = 2 + property_count as u16 + 1;
                    self.focused_position = Some(FocusedPosition::Comment(Position {
                        x: min(position.x, self.width.saturating_sub(1)),
                        y: min(
                            position.y.saturating_sub(comment_start_y),
                            self.comment_line_count.saturating_sub(1),
                        ),
                    }));
                }
            }
            FocusEvent::Unfocused => {
                self.focused_position = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                if property_count > 0 {
                    self.focused_position = Some(FocusedPosition::Property(0));
                } else {
                    self.focused_position = Some(FocusedPosition::Comment(Position { x, y: 0 }));
                }
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                self.focused_position = Some(FocusedPosition::Comment(Position {
                    x,
                    y: self.comment_line_count.saturating_sub(1),
                }));
            }
        }
    }

    pub fn update(&mut self, journal: &Journal, width: u16) {
        self.width = width;
        self.journal = journal.clone();
        self.widget_state
            .update(width, &journal.user, &journal.updated_on, &journal.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
        let property_count = self.journal.details.len();
        match &mut self.focused_position {
            None => {}
            Some(FocusedPosition::Property(index)) => {
                // FIXME: (現実的かはともかく)今回のupdateでpropertyリストが消えた場合を追加
                if *index >= property_count {
                    *index = property_count.saturating_sub(1);
                }
            }
            Some(FocusedPosition::Comment(position)) => {
                if position.x >= width {
                    position.x = width.saturating_sub(1);
                }
                if position.y >= self.comment_line_count {
                    position.y = self.comment_line_count.saturating_sub(1);
                }
            }
        }
    }

    pub fn create_widget<'a>(&'a self) -> JournalItemWidget<'a> {
        JournalItemWidget::new(
            &self.journal,
            &self.widget_state,
            self.focused_position.is_some(),
        )
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.journal.details.len() as u16 + 1 + self.comment_line_count
    }

    pub fn get_cursor_position(&self) -> Position {
        let property_count = self.journal.details.len();
        match self.focused_position {
            None => Position { x: 0, y: 0 },
            Some(FocusedPosition::Property(index)) => Position {
                x: 0,
                y: index as u16 + 2,
            },
            Some(FocusedPosition::Comment(position)) => Position {
                x: position.x,
                y: 2 + property_count as u16 + 1 + position.y,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        entities::{JournalDetail, JournalDetailAttr, JournalId},
        test_support::{local_datetime, render_snapshot},
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

    fn update_then_snapshot(
        name: &str,
        component: &mut JournalsListItemComponent,
        journal: &Journal,
        width: u16,
        expected_line_count: u16,
        expected_cursor: Position,
    ) {
        component.update(journal, width);
        assert_layout_contract(component, width, expected_line_count, expected_cursor);
        render_snapshot(
            name,
            width,
            component.line_count(width),
            component.create_widget(),
        );
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

    fn assert_leave_from_below(result: Option<EventProcessResult>, expected_x: u16) {
        match result {
            Some(EventProcessResult::CursorLeavedFromBelow { x }) => assert_eq!(x, expected_x),
            _ => panic!("expected cursor leave from below"),
        }
    }

    fn assert_leave_from_above(result: Option<EventProcessResult>, expected_x: u16) {
        match result {
            Some(EventProcessResult::CursorLeavedFromAbove { x }) => assert_eq!(x, expected_x),
            _ => panic!("expected cursor leave from above"),
        }
    }

    #[test]
    fn update_initial_state_is_unfocused_and_rendered() {
        let journal = create_journal(1, details(), notes());
        let mut component = JournalsListItemComponent::new(&journal);

        update_then_snapshot(
            "journals_list_item_component_initial_unfocused",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 0),
        );
    }

    #[test]
    fn update_changed_notes_updates_line_count_and_widget() {
        let initial_journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        let updated_journal = create_journal(1, one_detail(), updated_notes());

        update_then_snapshot(
            "journals_list_item_component_update_changed_notes",
            &mut component,
            &updated_journal,
            WIDE_WIDTH,
            7,
            Position::new(0, 0),
        );
    }

    #[test]
    fn update_narrower_width_wraps_notes_and_clamps_comment_cursor() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(31, 6),
        });

        update_then_snapshot(
            "journals_list_item_component_update_narrower_width",
            &mut component,
            &journal,
            NARROW_WIDTH,
            9,
            Position::new(17, 6),
        );
    }

    #[test]
    fn update_fewer_details_clamps_property_focus_to_last_detail() {
        let initial_journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(0, 3),
        });
        let updated_journal = create_journal(1, one_detail(), notes());

        update_then_snapshot(
            "journals_list_item_component_update_fewer_details",
            &mut component,
            &updated_journal,
            WIDE_WIDTH,
            8,
            Position::new(0, 2),
        );
    }

    #[test]
    fn focus_event_from_above_focuses_first_detail() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        update_then_snapshot(
            "journals_list_item_component_focus_from_above_to_detail",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 2),
        );
    }

    #[test]
    fn focus_event_from_above_without_details_focuses_first_note_line() {
        let journal = create_journal(1, vec![], notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        update_then_snapshot(
            "journals_list_item_component_focus_from_above_without_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            7,
            Position::new(6, 3),
        );
    }

    #[test]
    fn focus_event_from_below_with_notes_focuses_last_note_line() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        update_then_snapshot(
            "journals_list_item_component_focus_from_below_to_note",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(6, 8),
        );
    }

    #[test]
    fn focus_event_from_below_with_empty_notes_focuses_placeholder_line() {
        let journal = create_journal(1, details(), "");
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        update_then_snapshot(
            "journals_list_item_component_focus_from_below_without_notes",
            &mut component,
            &journal,
            WIDE_WIDTH,
            6,
            Position::new(6, 5),
        );
    }

    #[test]
    fn focused_position_above_details_focuses_first_detail() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 0),
        });

        update_then_snapshot(
            "journals_list_item_component_focused_above_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 2),
        );
    }

    #[test]
    fn focused_position_past_details_with_empty_notes_focuses_placeholder_line() {
        let journal = create_journal(1, details(), "");
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 99),
        });

        update_then_snapshot(
            "journals_list_item_component_focused_past_details_without_notes",
            &mut component,
            &journal,
            WIDE_WIDTH,
            6,
            Position::new(6, 5),
        );
    }

    #[test]
    fn focused_position_near_top_without_details_focuses_first_note_line() {
        let journal = create_journal(1, vec![], notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 0),
        });

        update_then_snapshot(
            "journals_list_item_component_focused_near_top_without_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            7,
            Position::new(6, 3),
        );
    }

    #[test]
    fn focused_position_past_note_with_details_focuses_last_note_line() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 99),
        });

        update_then_snapshot(
            "journals_list_item_component_focused_past_note_with_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(6, 8),
        );
    }

    #[test]
    fn focused_position_past_note_without_details_focuses_last_note_line() {
        let journal = create_journal(1, vec![], notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 99),
        });

        update_then_snapshot(
            "journals_list_item_component_focused_past_note_without_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            7,
            Position::new(6, 6),
        );
    }

    #[test]
    fn unfocused_removes_widget_focus() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        component.focus_event(FocusEvent::Unfocused);

        update_then_snapshot(
            "journals_list_item_component_unfocused",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 0),
        );
    }

    #[test]
    fn process_event_j_moves_between_details() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_j_between_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 3),
        );
    }

    #[test]
    fn process_event_j_on_last_detail_with_empty_notes_moves_to_placeholder_line() {
        let journal = create_journal(1, one_detail(), "");
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_j_on_last_detail_without_notes",
            &mut component,
            &journal,
            WIDE_WIDTH,
            5,
            Position::new(0, 4),
        );
    }

    #[test]
    fn process_event_j_moves_from_last_detail_to_first_note_line() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_j_from_detail_to_note",
            &mut component,
            &journal,
            WIDE_WIDTH,
            8,
            Position::new(0, 4),
        );
    }

    #[test]
    fn process_event_j_moves_between_note_lines() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });
        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        component.update(&journal, WIDE_WIDTH);

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_j_between_note_lines",
            &mut component,
            &journal,
            WIDE_WIDTH,
            8,
            Position::new(0, 5),
        );
    }

    #[test]
    fn process_event_j_on_last_note_line_returns_leave_from_below() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert_leave_from_below(result, 6);
        update_then_snapshot(
            "journals_list_item_component_process_j_on_last_note",
            &mut component,
            &journal,
            WIDE_WIDTH,
            8,
            Position::new(6, 7),
        );
    }

    #[test]
    fn process_event_k_moves_between_details() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        component.update(&journal, WIDE_WIDTH);

        let result = component.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_k_between_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 2),
        );
    }

    #[test]
    fn process_event_k_moves_from_first_note_line_to_last_detail() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 4),
        });

        let result = component.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_k_from_note_to_detail",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 3),
        );
    }

    #[test]
    fn process_event_k_moves_between_note_lines() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 5),
        });

        let result = component.process_event(key_event(KeyCode::Char('k')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_k_between_note_lines",
            &mut component,
            &journal,
            WIDE_WIDTH,
            8,
            Position::new(6, 4),
        );
    }

    #[test]
    fn process_event_k_on_first_note_line_without_details_returns_leave_from_above() {
        let journal = create_journal(1, vec![], notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('k')));
        assert_leave_from_above(result, 6);
        update_then_snapshot(
            "journals_list_item_component_process_k_on_first_note_without_details",
            &mut component,
            &journal,
            WIDE_WIDTH,
            7,
            Position::new(6, 3),
        );
    }

    #[test]
    fn process_event_k_on_first_detail_returns_leave_from_above() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('k')));

        assert_leave_from_above(result, 0);
        update_then_snapshot(
            "journals_list_item_component_process_k_on_first_detail",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 2),
        );
    }

    #[test]
    fn process_event_h_and_l_move_comment_x_and_clamp_at_edges() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(1, 3),
        });

        assert!(
            component
                .process_event(key_event(KeyCode::Char('h')))
                .is_none()
        );
        component.update(&journal, WIDE_WIDTH);
        assert_layout_contract(&component, WIDE_WIDTH, 8, Position::new(0, 4));
        assert!(
            component
                .process_event(key_event(KeyCode::Char('h')))
                .is_none()
        );
        component.update(&journal, WIDE_WIDTH);
        assert_layout_contract(&component, WIDE_WIDTH, 8, Position::new(0, 4));

        for _ in 0..40 {
            assert!(
                component
                    .process_event(key_event(KeyCode::Char('l')))
                    .is_none()
            );
            component.update(&journal, WIDE_WIDTH);
        }

        assert_layout_contract(&component, WIDE_WIDTH, 8, Position::new(31, 4));
        render_snapshot(
            "journals_list_item_component_process_h_l_comment_x",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn process_event_while_unfocused_does_nothing() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        update_then_snapshot(
            "journals_list_item_component_process_event_unfocused",
            &mut component,
            &journal,
            WIDE_WIDTH,
            9,
            Position::new(0, 0),
        );
    }

    #[test]
    fn update_removed_notes_keeps_comment_focus_on_placeholder_line() {
        let initial_journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });
        let updated_journal = create_journal(1, details(), "");

        component.update(&updated_journal, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 6, Position::new(6, 5));
        render_snapshot(
            "journals_list_item_component_update_removed_notes",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }
}
