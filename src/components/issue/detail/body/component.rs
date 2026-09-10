use super::focus_state;
use super::focus_state::FocusState;
use super::widget::{BodyWidget, BodyWidgetState};
use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::IssueAggregate;
use crate::vos::IssueId;

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
    EditRequested { id: IssueId, body: String },
}

pub struct BodyComponent {
    id: IssueId,
    focus_state: FocusState,
    body: String,
    widget_state: BodyWidgetState,
}

impl BodyComponent {
    pub fn new(id: impl Into<IssueId>) -> Self {
        Self {
            id: id.into(),
            focus_state: FocusState::new(),
            body: String::new(),
            widget_state: BodyWidgetState::new(),
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
                    id: self.id,
                    body: self.body.clone(),
                },
            })
    }

    pub fn focus_event(&mut self, event: super::FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, issue: &IssueAggregate, width: u16) {
        self.body = issue.description.clone();
        self.widget_state.update(width, &self.body);
        let height = self.widget_state.line_count(width) as u16;
        self.focus_state.update(width, height);
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.widget_state.line_count(width) as u16
    }

    pub fn create_widget<'a>(&'a self) -> BodyWidget<'a> {
        BodyWidget::new(&self.widget_state, self.focus_state.is_focused())
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

#[cfg(test)]
mod tests {
    use super::super::focus_state::FocusEvent;
    use super::*;
    use crate::test_support::{render_snapshot, sample_issue_aggregate};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Position;

    const ISSUE_ID: u16 = 1;
    const WIDE_WIDTH: u16 = 32;
    const NARROW_WIDTH: u16 = 16;
    const WIDE_LINE_COUNT: u16 = 6;
    const NARROW_LINE_COUNT: u16 = 8;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn issue_with_body(body: &str) -> IssueAggregate {
        let mut issue = sample_issue_aggregate(
            ISSUE_ID,
            "Body component issue",
            1.into(),
            Some(1),
            None,
            None,
            0,
        );
        issue.description = body.to_string();
        issue
    }

    fn wrapping_body() -> &'static str {
        "# Heading\n\n- first item\n- second item\n\nParagraph text that should wrap."
    }

    fn edited_body() -> &'static str {
        "# Updated\n\nA different issue body is now rendered and used for editing."
    }

    fn updated_component(width: u16, body: &str) -> BodyComponent {
        let issue = issue_with_body(body);
        let mut component = BodyComponent::new(ISSUE_ID);
        component.update(&issue, width);
        component
    }

    fn assert_layout_contract(
        component: &BodyComponent,
        width: u16,
        line_count: u16,
        cursor: Position,
    ) {
        assert_eq!(component.line_count(width), line_count);
        assert_eq!(component.get_cursor_position(), cursor);
    }

    #[test]
    fn focus_event_position_after_update_is_reflected_in_widget_and_cursor() {
        let mut component = updated_component(WIDE_WIDTH, wrapping_body());

        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 2),
        });

        assert_layout_contract(&component, WIDE_WIDTH, WIDE_LINE_COUNT, Position::new(6, 2));
        render_snapshot(
            "body_component_focus_position_after_update",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn process_event_j_moves_cursor_down_and_keeps_widget_focused() {
        let mut component = updated_component(WIDE_WIDTH, wrapping_body());
        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 1),
        });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_layout_contract(&component, WIDE_WIDTH, WIDE_LINE_COUNT, Position::new(6, 2));
        render_snapshot(
            "body_component_process_j",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn process_event_j_on_bottom_line_returns_leave_from_below_and_keeps_state() {
        let mut component = updated_component(WIDE_WIDTH, wrapping_body());
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        match result {
            Some(EventProcessResult::CursorLeavedFromBelow { x }) => {
                assert_eq!(x, 6);
            }
            _ => panic!("expected cursor leave from below"),
        }
        assert_layout_contract(&component, WIDE_WIDTH, WIDE_LINE_COUNT, Position::new(6, 5));
        render_snapshot(
            "body_component_process_j_on_bottom_line",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn process_event_k_on_top_line_returns_leave_from_above_and_keeps_state() {
        let mut component = updated_component(WIDE_WIDTH, wrapping_body());
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        let result = component.process_event(key_event(KeyCode::Char('k')));

        match result {
            Some(EventProcessResult::CursorLeavedFromAbove { x }) => {
                assert_eq!(x, 6);
            }
            _ => panic!("expected cursor leave from above"),
        }
        assert_layout_contract(&component, WIDE_WIDTH, WIDE_LINE_COUNT, Position::new(6, 0));
        render_snapshot(
            "body_component_process_k_on_top_line",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn process_event_e_returns_current_issue_body_without_moving_cursor() {
        let body = wrapping_body();
        let mut component = updated_component(WIDE_WIDTH, body);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(5, 1),
        });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested {
                id,
                body: edit_body,
            }) => {
                assert_eq!(id, ISSUE_ID);
                assert_eq!(edit_body, body);
            }
            _ => panic!("expected edit request"),
        }
        assert_layout_contract(&component, WIDE_WIDTH, WIDE_LINE_COUNT, Position::new(5, 1));
        render_snapshot(
            "body_component_process_e",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn update_to_narrower_width_tracks_line_count_and_current_cursor_clamp() {
        let issue = issue_with_body(wrapping_body());
        let mut component = BodyComponent::new(ISSUE_ID);
        component.update(&issue, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(31, 5),
        });

        component.update(&issue, NARROW_WIDTH);

        assert_layout_contract(
            &component,
            NARROW_WIDTH,
            NARROW_LINE_COUNT,
            Position::new(16, 5),
        );
        render_snapshot(
            "body_component_update_narrower_width",
            NARROW_WIDTH,
            component.line_count(NARROW_WIDTH),
            component.create_widget(),
        );
    }

    #[test]
    fn update_to_different_issue_body_changes_widget_and_edit_body() {
        let mut component = updated_component(WIDE_WIDTH, wrapping_body());
        let issue = issue_with_body(edited_body());
        component.focus_event(FocusEvent::Focused {
            position: Position::new(6, 2),
        });

        component.update(&issue, WIDE_WIDTH);
        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested { id, body }) => {
                assert_eq!(id, ISSUE_ID);
                assert_eq!(body, edited_body());
            }
            _ => panic!("expected edit request"),
        }
        assert_layout_contract(&component, WIDE_WIDTH, 4, Position::new(6, 2));
        render_snapshot(
            "body_component_update_different_issue_body",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(),
        );
    }
}
