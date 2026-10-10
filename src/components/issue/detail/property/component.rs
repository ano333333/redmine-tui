use std::cell::RefCell;
use std::rc::Rc;

use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::PropertyWidget;
use ratatui::layout::Position;

use crate::entities::{IssueStatus, IssueView};
use crate::platform::input::InputEvent;
use crate::stores::{Dispatcher, NoticeAction, NoticeId, Store};
use crate::vos::IssueId;

pub struct PropertyComponent {
    id: IssueId,
    focus_state: FocusState,
}

impl PropertyComponent {
    pub fn new(id: impl Into<IssueId>) -> Self {
        Self {
            id: id.into(),
            focus_state: FocusState::new(),
        }
    }

    /// 編集できない属性のpopupを開く要求は、Noticeを出して`Handled`に置き換える。
    pub fn process_event(
        &mut self,
        event: InputEvent,
        dispatcher: Rc<RefCell<Dispatcher>>,
    ) -> Option<EventProcessResult> {
        let result = self.focus_state.process_event(event)?;
        let blocked_property = {
            let dispatcher_ref = dispatcher.borrow();
            let store = dispatcher_ref.store();
            match result {
                EventProcessResult::OpenPriorityPopup
                    if !store.is_issue_priority_editable(self.id) =>
                {
                    Some("優先度")
                }
                EventProcessResult::OpenStartDatePopup
                    if !store.are_issue_dates_editable(self.id) =>
                {
                    Some("開始日")
                }
                EventProcessResult::OpenDueDatePopup
                    if !store.are_issue_dates_editable(self.id) =>
                {
                    Some("期日")
                }
                EventProcessResult::OpenDoneRatioPopup
                    if !store.is_issue_done_ratio_editable(self.id) =>
                {
                    Some("進捗率")
                }
                _ => None,
            }
        };
        let Some(property) = blocked_property else {
            return Some(result);
        };
        dispatcher.borrow_mut().dispatch(NoticeAction::Push {
            id: NoticeId::new(),
            message: format!(
                "Issue #{}の{property}は子Issueから計算されるため編集できません",
                self.id
            ),
        });
        Some(EventProcessResult::Handled)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, width: u16) {
        self.focus_state
            .update(PropertyWidget::is_two_column(width));
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        let (issue, _) = store.get_issue(self.id);
        let paragraph = create_property_widget(
            issue,
            store,
            store.get_issue_status(issue.status_id()),
            None,
        );
        paragraph.line_count(width) as u16
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> PropertyWidget<'a> {
        let (issue, _) = store.get_issue(self.id);
        create_property_widget(
            issue,
            store,
            store.get_issue_status(issue.status_id()),
            self.focus_state.focused_y(),
        )
    }

    pub fn get_cursor_position(&self, width: u16) -> Position {
        self.focus_state.get_cursor_position(width)
    }
}

fn create_property_widget<'a>(
    issue: IssueView<'a>,
    store: &'a Store,
    issue_status: Option<&'a IssueStatus>,
    focused_y: Option<u16>,
) -> PropertyWidget<'a> {
    let author = store
        .get_user(issue.author_id())
        .map(|user| user.name.as_str())
        .unwrap_or("(unknown)");
    let assigned_to = issue
        .assigned_to_id()
        .and_then(|user_id| store.get_user(user_id))
        .map(|user| user.name.as_str());
    let priority = store
        .get_priority(issue.priority_id())
        .map(|priority| priority.name.as_str())
        .unwrap_or("(unknown)");
    let project = store
        .get_project(issue.project_id())
        .map(|project| project.name.as_str())
        .unwrap_or("(unknown)");
    let tracker = store
        .get_tracker(issue.tracker_id())
        .map(|tracker| tracker.name.as_str())
        .unwrap_or("(unknown)");
    let target_version = issue
        .target_version_id()
        .and_then(|target_version_id| store.get_target_version(target_version_id))
        .map(|target_version| target_version.name.as_str());
    let category = match issue.category_id() {
        Some(category_id) => store
            .get_category(category_id)
            .map(|category| category.name.as_str())
            .unwrap_or("(unknown)"),
        None => "-",
    };
    PropertyWidget::new(
        issue.id(),
        author,
        issue.created_on(),
        issue.updated_on(),
        issue_status
            .map(|issue_status| issue_status.name.as_str())
            .unwrap_or("(unknown)"),
        tracker,
        priority,
        project,
        assigned_to,
        target_version,
        issue.start_date(),
        issue.due_date(),
        issue.done_ratio(),
        issue.estimated_hours(),
        issue.total_spent_hours(),
        category,
        focused_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};
    use crate::stores::Action;
    use crate::test_support::{dispatch_sample_masters, render_snapshot, sync_sample_masters};
    use crate::widgets::gutter::GUTTER_WIDTH;
    use ratatui::layout::Position;

    /// 値列の開始桁。WIDTH(=40)は2カラムに畳まない幅なので、常に左カラム。
    /// 縦線の字下げ分を含む。
    const VALUE_X: u16 = GUTTER_WIDTH + PropertyWidget::VALUE_X;

    const ISSUE_ID: u16 = 1;
    const WIDTH: u16 = 40;
    /// プロパティの項目数。
    const FIELD_COUNT: u16 = 15;
    /// WIDTH(=40)は2カラムに畳まない幅。15項目 + 縦線を引かない末尾の余白1行。
    const PROPERTY_LINE_COUNT: u16 = FIELD_COUNT + 1;
    const TARGET_VERSION_LINE: u16 = 8;
    const START_DATE_LINE: u16 = 9;
    const DUE_DATE_LINE: u16 = 10;
    const DONE_RATIO_LINE: u16 = 11;
    const TOTAL_SPENT_HOURS_LINE: u16 = 13;
    const CATEGORY_LINE: u16 = 14;
    /// フォーカス可能な最後の項目インデックス(末尾の余白行は含まない)。
    const FOCUSABLE_LAST_LINE: u16 = FIELD_COUNT - 1;

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn store_with_property_issue() -> Store {
        let mut store = Store::new();
        sync_sample_masters(&mut store);
        crate::test_support::load_issue(&mut store, crate::test_support::sample_open_child_issue());
        store
    }

    fn dispatcher_with_issue(actions: [Action; 2]) -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            dispatch_sample_masters(&mut dispatcher_ref);
            for action in actions {
                dispatcher_ref.dispatch(action);
            }
            consume_all(&mut dispatcher_ref);
        }
        dispatcher
    }

    fn consume_all(dispatcher: &mut Dispatcher) {
        while dispatcher.consume_actinos_len() > 0 {
            dispatcher.consume_action();
        }
    }

    fn dispatcher_with_property_issue() -> Rc<RefCell<Dispatcher>> {
        dispatcher_with_issue(crate::test_support::fetch_issue_actions(
            crate::test_support::sample_open_child_issue(),
        ))
    }

    /// Issue 3を子Issue 1・2とともに取得済みにする。
    fn dispatcher_with_parent_issue() -> Rc<RefCell<Dispatcher>> {
        dispatcher_with_issue(crate::test_support::fetch_sample_parent_issue_actions(
            vec![],
        ))
    }

    const PRIORITY_LINE: u16 = 5;

    fn store_with_missing_issue_status() -> Store {
        let mut store = store_with_property_issue();
        store.consume_action(Action::SyncIssueStatuses {
            issue_statuses: vec![],
        });
        store
    }

    fn assert_layout_contract(component: &PropertyComponent, store: &Store, cursor: Position) {
        assert_eq!(component.line_count(store, WIDTH), PROPERTY_LINE_COUNT);
        assert_eq!(component.get_cursor_position(WIDTH), cursor);
    }

    // FIXME: author / priority / project / tracker / category の欠損も検証する際は、
    // master data lookup の重複したセットアップを避けるため、このテストへ統合する。
    #[test]
    fn missing_status_is_rendered_without_panicking() {
        let store = store_with_missing_issue_status();
        let component = PropertyComponent::new(ISSUE_ID);

        assert_eq!(component.line_count(&store, WIDTH), PROPERTY_LINE_COUNT);
        render_snapshot(
            "property_component_missing_status",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn focus_event_from_above_is_reflected_in_widget_and_cursor() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);

        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert_layout_contract(&component, &store, Position::new(VALUE_X, 0));
        render_snapshot(
            "property_component_focus_from_above",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn focus_event_from_below_preserves_current_focusable_line_count() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);

        component.focus_event(FocusEvent::CursorEnteredFromBelow);

        assert_layout_contract(
            &component,
            &store,
            Position::new(VALUE_X, FOCUSABLE_LAST_LINE),
        );
        render_snapshot(
            "property_component_focus_from_below",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_j_updates_widget_focus_and_cursor() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, 1),
        );
        render_snapshot(
            "property_component_process_j",
            WIDTH,
            component.line_count(dispatcher.borrow().store(), WIDTH),
            component.create_widget(dispatcher.borrow().store()),
        );
    }

    #[test]
    fn process_event_e_returns_status_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..3 {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenIssueStatusPopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, 3),
        );
        render_snapshot(
            "property_component_process_e_on_status",
            WIDTH,
            component.line_count(dispatcher.borrow().store(), WIDTH),
            component.create_widget(dispatcher.borrow().store()),
        );
    }

    #[test]
    fn process_event_e_returns_assigned_to_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..7 {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenAssignedToPopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, 7),
        );
    }

    #[test]
    fn process_event_e_returns_target_version_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..TARGET_VERSION_LINE {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenTargetVersionPopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, TARGET_VERSION_LINE),
        );
    }

    #[test]
    fn process_event_e_returns_done_ratio_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..DONE_RATIO_LINE {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenDoneRatioPopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, DONE_RATIO_LINE),
        );
    }

    #[test]
    fn process_event_e_returns_start_date_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..START_DATE_LINE {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenStartDatePopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, START_DATE_LINE),
        );
    }

    #[test]
    fn process_event_e_returns_due_date_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..DUE_DATE_LINE {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(result, Some(EventProcessResult::OpenDueDatePopup)));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, DUE_DATE_LINE),
        );
    }

    #[test]
    fn process_event_e_returns_spent_time_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromBelow);
        component.process_event(key_event(KeyCode::Char('k')), dispatcher.clone());

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenSpentTimeInputPopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, TOTAL_SPENT_HOURS_LINE),
        );
        render_snapshot(
            "property_component_process_e_on_spent_time",
            WIDTH,
            component.line_count(dispatcher.borrow().store(), WIDTH),
            component.create_widget(dispatcher.borrow().store()),
        );
    }

    #[test]
    fn process_event_e_returns_category_popup_result_without_changing_widget_focus() {
        let dispatcher = dispatcher_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromBelow);

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenCategoryPopup)
        ));
        assert_layout_contract(
            &component,
            dispatcher.borrow().store(),
            Position::new(VALUE_X, CATEGORY_LINE),
        );
    }

    /// `id`のIssueのプロパティ先頭行から`line`行下へフォーカスを移し、`e`を押す。
    fn press_e_on_line(
        dispatcher: Rc<RefCell<Dispatcher>>,
        id: u16,
        line: u16,
    ) -> Option<EventProcessResult> {
        let mut component = PropertyComponent::new(id);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..line {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }
        component.process_event(key_event(KeyCode::Char('e')), dispatcher)
    }

    #[test]
    fn process_event_e_on_derived_property_of_issue_with_children_pushes_notice() {
        for (line, expected) in [
            (
                PRIORITY_LINE,
                "Issue #3の優先度は子Issueから計算されるため編集できません",
            ),
            (
                START_DATE_LINE,
                "Issue #3の開始日は子Issueから計算されるため編集できません",
            ),
            (
                DUE_DATE_LINE,
                "Issue #3の期日は子Issueから計算されるため編集できません",
            ),
            (
                DONE_RATIO_LINE,
                "Issue #3の進捗率は子Issueから計算されるため編集できません",
            ),
        ] {
            let dispatcher = dispatcher_with_parent_issue();

            let result = press_e_on_line(dispatcher.clone(), 3, line);

            assert!(matches!(result, Some(EventProcessResult::Handled)));
            consume_all(&mut dispatcher.borrow_mut());
            let messages: Vec<_> = dispatcher
                .borrow()
                .store()
                .get_notices()
                .iter()
                .map(|notice| notice.message.clone())
                .collect();
            assert_eq!(messages, [expected]);
        }
    }

    #[test]
    fn process_event_e_on_priority_of_issue_without_children_opens_popup() {
        let dispatcher = dispatcher_with_property_issue();

        let result = press_e_on_line(dispatcher.clone(), ISSUE_ID, PRIORITY_LINE);

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenPriorityPopup)
        ));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn process_event_e_on_category_of_issue_with_children_opens_popup() {
        let dispatcher = dispatcher_with_parent_issue();

        let result = press_e_on_line(dispatcher.clone(), 3, CATEGORY_LINE);

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenCategoryPopup)
        ));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }
}
