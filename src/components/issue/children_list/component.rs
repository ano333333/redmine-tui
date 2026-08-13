use crossterm::event::Event;
use ratatui::layout::Position;

use crate::app::Store;
use crate::vos::EntityIdValue;

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
                store.get_issue(id.get()).is_some_and(|(issue, _)| {
                    let issue_status = store.get_issue_status(issue.status_id);
                    issue_status.is_closed
                })
            })
            .count() as u16;
        let child_opened_num = child_all_num - child_closed_num;

        let children: Vec<_> = issue
            .child_ids
            .iter()
            .filter_map(|id| store.get_issue(id.get()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Action;
    use crate::test_support::render_snapshot;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Position;

    const ISSUE_ID: u16 = 3;
    const WIDTH: u16 = 80;
    const CHILDREN_LIST_LINE_COUNT: u16 = 5;

    fn store_with_parent_and_children() -> Store {
        let mut store = Store::new();
        store.consume_action(Action::LoadUsers);
        store.consume_action(Action::LoadIssueStatuses);
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::LoadIssue { id: 2.into() });
        store.consume_action(Action::LoadIssue {
            id: ISSUE_ID.into(),
        });
        store
    }

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn assert_layout_contract(component: &ChildrenListComponent, store: &Store, cursor: Position) {
        assert_eq!(component.line_count(store), CHILDREN_LIST_LINE_COUNT);
        assert_eq!(component.get_cursor_position(), cursor);
    }

    #[test]
    fn focus_event_from_above_is_reflected_in_widget_and_cursor() {
        let store = store_with_parent_and_children();
        let mut component = ChildrenListComponent::new(ISSUE_ID.into());
        component.update(&store);

        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert_layout_contract(&component, &store, Position { x: 0, y: 2 });
        render_snapshot(
            "children_list_component_focus_from_above",
            WIDTH,
            component.line_count(&store),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_j_moves_focus_to_second_child_and_updates_widget_and_cursor() {
        let store = store_with_parent_and_children();
        let mut component = ChildrenListComponent::new(ISSUE_ID.into());
        component.update(&store);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = component.process_event(&key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_layout_contract(&component, &store, Position { x: 0, y: 3 });
        render_snapshot(
            "children_list_component_process_j",
            WIDTH,
            component.line_count(&store),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_j_on_bottom_child_returns_leave_from_below_and_keeps_widget_coherent() {
        let store = store_with_parent_and_children();
        let mut component = ChildrenListComponent::new(ISSUE_ID.into());
        component.update(&store);
        component.focus_event(FocusEvent::CursorEnteredFromBelow);

        let result = component.process_event(&key_event(KeyCode::Char('j')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
        assert_layout_contract(&component, &store, Position { x: 0, y: 3 });
        render_snapshot(
            "children_list_component_process_j_on_bottom",
            WIDTH,
            component.line_count(&store),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_k_on_top_child_returns_leave_from_above_and_keeps_widget_coherent() {
        let store = store_with_parent_and_children();
        let mut component = ChildrenListComponent::new(ISSUE_ID.into());
        component.update(&store);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = component.process_event(&key_event(KeyCode::Char('k')));

        assert!(matches!(
            result,
            Some(EventProcessResult::CursorLeavedFromAbove)
        ));
        assert_layout_contract(&component, &store, Position { x: 0, y: 2 });
        render_snapshot(
            "children_list_component_process_k_on_top",
            WIDTH,
            component.line_count(&store),
            component.create_widget(&store),
        );
    }

    #[test]
    fn unfocused_after_update_removes_widget_focus() {
        let store = store_with_parent_and_children();
        let mut component = ChildrenListComponent::new(ISSUE_ID.into());
        component.update(&store);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        component.focus_event(FocusEvent::Unfocused);

        assert_eq!(component.line_count(&store), CHILDREN_LIST_LINE_COUNT);
        render_snapshot(
            "children_list_component_unfocused",
            WIDTH,
            component.line_count(&store),
            component.create_widget(&store),
        );
    }
}
