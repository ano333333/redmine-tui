use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::IssueStatusExt;
use crate::inputs::native::convert_key;
use crate::stores::Store;
use crate::vos::IssueId;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{ChildIssueRow, ChildrenListWidget};

pub struct ChildrenListComponent {
    id: IssueId,
    focus_state: FocusState,
}

impl ChildrenListComponent {
    pub fn new(id: impl Into<IssueId>) -> Self {
        Self {
            id: id.into(),
            focus_state: FocusState::new(),
        }
    }

    pub fn update(&mut self, store: &Store) {
        let (issue, _) = store.get_issue(self.id);
        self.focus_state.update(&issue.child_ids);
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> ChildrenListWidget<'a> {
        let (issue, _) = store.get_issue(self.id);

        let (child_all_num, child_closed_num, child_opened_num) =
            child_status_counts(store, &issue.child_ids);
        let children = create_child_rows(store, &issue.child_ids);

        ChildrenListWidget::new(
            child_all_num,
            child_closed_num,
            child_opened_num,
            children,
            self.focus_state.focused_index(),
        )
    }

    pub fn line_count(&self, store: &Store) -> u16 {
        let (issue, _) = store.get_issue(self.id);
        2 + (issue.child_ids.len() as u16) + 1
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn process_event(&mut self, event: &Event) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };
        // 上位Componentの入力契約がcrosstermの間だけ、共通入力へ移行済みのFocusStateとの境界で変換する。
        let event = convert_key(*key)?;
        self.focus_state.process_event(&event)
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

fn child_status_counts(store: &Store, child_ids: &[IssueId]) -> (u16, u16, u16) {
    let child_all_num = child_ids.len() as u16;
    let child_closed_num = child_ids
        .iter()
        .filter(|id| {
            store.try_get_issue_state(**id).is_some() && {
                let (issue, _) = store.get_issue(**id);
                store
                    .get_issue_status(issue.issue.status_id)
                    .is_closed_status()
            }
        })
        .count() as u16;
    let child_opened_num = child_all_num - child_closed_num;

    (child_all_num, child_closed_num, child_opened_num)
}

fn create_child_rows<'a>(store: &'a Store, child_ids: &[IssueId]) -> Vec<ChildIssueRow<'a>> {
    child_ids
        .iter()
        .filter(|id| store.try_get_issue_state(**id).is_some())
        .map(|id| store.get_issue(*id))
        .map(|(issue, _)| ChildIssueRow {
            issue,
            issue_status: store.get_issue_status(issue.issue.status_id),
            assigned_to_name: issue
                .assigned_to_id
                .and_then(|user_id| store.get_user(user_id))
                .map(|user| user.name.as_str()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::IssueStatus;
    use crate::stores::{Action, IssueAction};
    use crate::test_support::{render_snapshot, sync_fixture_entities};
    use crate::widgets::gutter::GUTTER_WIDTH;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Position;

    const ISSUE_ID: u16 = 3;
    const WIDTH: u16 = 80;
    const CHILDREN_LIST_LINE_COUNT: u16 = 5;

    fn store_with_parent_and_children() -> Store {
        let mut store = Store::new();
        sync_fixture_entities(&mut store);
        store.consume_action(IssueAction::Load { id: 1.into() }.into());
        store.consume_action(IssueAction::Load { id: 2.into() }.into());
        store.consume_action(
            IssueAction::Load {
                id: ISSUE_ID.into(),
            }
            .into(),
        );
        store
    }

    fn store_with_parent_and_unknown_status_child() -> Store {
        let mut store = store_with_parent_and_children();
        store.consume_action(Action::SyncIssueStatuses {
            issue_statuses: vec![IssueStatus {
                id: 3.into(),
                name: "進行中(accepted)".to_string(),
                is_closed: false,
            }],
        });
        store
    }

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    /// `cursor` は一覧内の位置で渡す。縦線による字下げはここで足す。
    fn assert_layout_contract(component: &ChildrenListComponent, store: &Store, cursor: Position) {
        assert_eq!(component.line_count(store), CHILDREN_LIST_LINE_COUNT);
        assert_eq!(
            component.get_cursor_position(),
            Position {
                x: cursor.x + GUTTER_WIDTH,
                y: cursor.y,
            }
        );
    }

    #[test]
    fn create_widget_keeps_unknown_status_child_and_counts_it_as_open() {
        let store = store_with_parent_and_unknown_status_child();
        let mut component = ChildrenListComponent::new(ISSUE_ID);
        component.update(&store);
        let (parent, _) = store.get_issue(ISSUE_ID);
        let children = create_child_rows(&store, &parent.child_ids);
        let (child_all_num, child_closed_num, child_opened_num) =
            child_status_counts(&store, &parent.child_ids);

        assert!(store.get_issue_status(5.into()).is_none());
        assert_eq!(children.len(), 2);
        assert_eq!(child_closed_num, 0);
        assert_eq!(child_opened_num, 2);
        assert_eq!(child_all_num, child_closed_num + child_opened_num);
        assert_eq!(component.line_count(&store), CHILDREN_LIST_LINE_COUNT);
        render_snapshot(
            "children_list_component_unknown_status_child",
            WIDTH,
            component.line_count(&store),
            component.create_widget(&store),
        );
    }

    #[test]
    fn focus_event_from_above_is_reflected_in_widget_and_cursor() {
        let store = store_with_parent_and_children();
        let mut component = ChildrenListComponent::new(ISSUE_ID);
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
        let mut component = ChildrenListComponent::new(ISSUE_ID);
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
        let mut component = ChildrenListComponent::new(ISSUE_ID);
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
        let mut component = ChildrenListComponent::new(ISSUE_ID);
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
        let mut component = ChildrenListComponent::new(ISSUE_ID);
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
