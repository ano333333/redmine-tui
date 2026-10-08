use ratatui::layout::Position;

use crate::entities::{IssueChild, IssueStatusExt};
use crate::platform::input::InputEvent;
use crate::stores::Store;
use crate::vos::IssueId;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::{ChildIssueDetail, ChildIssueRow, ChildrenListWidget};

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
        let child_ids: Vec<IssueId> = store
            .get_issue_children(self.id)
            .iter()
            .map(|child| child.id)
            .collect();
        self.focus_state.update(&child_ids);
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> ChildrenListWidget<'a> {
        let children = create_child_rows(store, store.get_issue_children(self.id));
        let (child_all_num, child_closed_num, child_opened_num) = child_status_counts(&children);

        ChildrenListWidget::new(
            child_all_num,
            child_closed_num,
            child_opened_num,
            children,
            self.focus_state.focused_index(),
        )
    }

    pub fn line_count(&self, store: &Store) -> u16 {
        2 + (store.get_issue_children(self.id).len() as u16) + 1
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

/// 詳細が未取得の子はstatusが分からないため、未完了として数える。
fn child_status_counts(children: &[ChildIssueRow]) -> (u16, u16, u16) {
    let child_all_num = children.len() as u16;
    let child_closed_num = children
        .iter()
        .filter(|child| {
            child
                .detail
                .as_ref()
                .is_some_and(|detail| detail.issue_status.is_closed_status())
        })
        .count() as u16;
    let child_opened_num = child_all_num - child_closed_num;

    (child_all_num, child_closed_num, child_opened_num)
}

// FIXME: 行の並びは取得した子一覧で決めるが、値は詳細を取得済みの子だけIssueViewで補っている。
// 詳細が未取得の子はIDと題名しか表示できず、子一覧が持つトラッカーと孫の一覧も使っていない。
// 子一覧の情報から子Issueとして表示する形へ見直す。
fn create_child_rows<'a>(store: &'a Store, children: &'a [IssueChild]) -> Vec<ChildIssueRow<'a>> {
    children
        .iter()
        .map(|child| {
            if store.try_get_issue_state(child.id).is_none() {
                return ChildIssueRow {
                    id: child.id,
                    subject: &child.subject,
                    detail: None,
                };
            }
            let (issue, _) = store.get_issue(child.id);
            ChildIssueRow {
                id: child.id,
                subject: issue.subject(),
                detail: Some(ChildIssueDetail {
                    issue_status: store.get_issue_status(issue.status_id()),
                    assigned_to_name: issue
                        .assigned_to_id()
                        .and_then(|user_id| store.get_user(user_id))
                        .map(|user| user.name.as_str()),
                    start_date: issue.start_date(),
                    due_date: issue.due_date(),
                    done_ratio: issue.done_ratio(),
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::IssueStatus;
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};
    use crate::stores::{Action, IssueAction};
    use crate::test_support::{render_snapshot, sync_sample_masters};
    use crate::widgets::gutter::GUTTER_WIDTH;
    use ratatui::layout::Position;

    const ISSUE_ID: u16 = 3;
    const WIDTH: u16 = 80;
    const CHILDREN_LIST_LINE_COUNT: u16 = 5;

    fn store_with_parent_and_children() -> Store {
        let mut store = Store::new();
        sync_sample_masters(&mut store);
        crate::test_support::load_issue(&mut store, crate::test_support::sample_open_child_issue());
        crate::test_support::load_issue(
            &mut store,
            crate::test_support::sample_closed_child_issue(),
        );
        for action in crate::test_support::fetch_sample_parent_issue_actions(vec![]) {
            store.consume_action(action);
        }
        store
    }

    #[test]
    fn child_rows_show_unsaved_edits_of_loaded_children() {
        let mut store = store_with_parent_and_children();
        store.consume_action(
            IssueAction::UpdateDoneRatio {
                id: 1.into(),
                done_ratio: 40,
            }
            .into(),
        );

        let rows = create_child_rows(&store, store.get_issue_children(ISSUE_ID));

        let row = rows
            .iter()
            .find(|row| row.id == crate::vos::IssueId::from(1))
            .expect("child 1 is listed");
        assert_eq!(
            row.detail.as_ref().map(|detail| detail.done_ratio),
            Some(40)
        );
    }

    #[test]
    fn unloaded_children_are_listed_with_the_fetched_subject_and_counted_as_open() {
        let mut store = Store::new();
        sync_sample_masters(&mut store);
        crate::test_support::load_issue(
            &mut store,
            crate::test_support::sample_closed_child_issue(),
        );
        for action in crate::test_support::fetch_sample_parent_issue_actions(vec![]) {
            store.consume_action(action);
        }

        let rows = create_child_rows(&store, store.get_issue_children(ISSUE_ID));

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].subject, "issue1");
        assert!(rows[0].detail.is_none());
        assert!(rows[1].detail.is_some());
        assert_eq!(child_status_counts(&rows), (2, 1, 1));
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

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
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
        let children = create_child_rows(&store, store.get_issue_children(ISSUE_ID));
        let (child_all_num, child_closed_num, child_opened_num) = child_status_counts(&children);

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

        assert!(matches!(result, Some(EventProcessResult::Handled)));
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
