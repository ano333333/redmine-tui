use super::focus_state::{EventProcessResult, FocusEvent, FocusState};
use super::widget::PropertyWidget;
use crossterm::event::Event;
use ratatui::layout::Position;

use crate::app::Store;
use crate::entities::{Issue, IssueStatus};
use crate::vos::EntityIdValue;

pub struct PropertyComponent {
    id: u16,
    focus_state: FocusState,
}

impl PropertyComponent {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            focus_state: FocusState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state.process_event(event)
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some((issue, _)) = store.get_issue(self.id) {
            let paragraph =
                create_property_widget(issue, store, store.get_issue_status(issue.status_id), None);
            paragraph.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> PropertyWidget<'a> {
        let (issue, _) = store.get_issue(self.id).unwrap();
        create_property_widget(
            issue,
            store,
            store.get_issue_status(issue.status_id),
            self.focus_state.focused_y(),
        )
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

fn create_property_widget<'a>(
    issue: &'a Issue,
    store: &'a Store,
    issue_status: &'a IssueStatus,
    focused_y: Option<u16>,
) -> PropertyWidget<'a> {
    let author = store
        .get_user(issue.author_id)
        .map(|user| user.name.as_str())
        .unwrap_or("(unknown)");
    let assigned_to = issue
        .assigned_to_id
        .and_then(|user_id| store.get_user(user_id))
        .map(|user| user.name.as_str());
    let priority = store
        .get_priority(issue.priority_id)
        .map(|priority| priority.name.as_str())
        .unwrap_or("(unknown)");
    let project = store
        .get_project(issue.project_id)
        .map(|project| project.name.as_str())
        .unwrap_or("(unknown)");
    let tracker = store
        .get_tracker(issue.tracker_id.get())
        .map(|tracker| tracker.name.as_str())
        .unwrap_or("(unknown)");
    let target_version = issue
        .target_version_id
        .and_then(|target_version_id| store.get_target_version(target_version_id))
        .map(|target_version| target_version.name.as_str());
    let component = store
        .get_component(issue.component_id)
        .map(|component| component.name.as_str())
        .unwrap_or("(unknown)");
    PropertyWidget::new(
        issue.id.get(),
        author,
        issue.created_on,
        issue.updated_on,
        issue_status.name.as_str(),
        tracker,
        priority,
        project,
        assigned_to,
        target_version,
        issue.start_date,
        issue.due_date,
        issue.done_ratio,
        issue.estimated_hours,
        issue.total_spent_hours,
        component,
        focused_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Action;
    use crate::test_support::render_snapshot;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Position;

    const ISSUE_ID: u16 = 1;
    const WIDTH: u16 = 40;
    const PROPERTY_LINE_COUNT: u16 = 15;
    const TARGET_VERSION_LINE: u16 = 8;
    const DONE_RATIO_LINE: u16 = 11;
    const TOTAL_SPENT_HOURS_LINE: u16 = 13;
    const FOCUSABLE_LAST_LINE: u16 = PROPERTY_LINE_COUNT - 1;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn store_with_property_issue() -> Store {
        let mut store = Store::new();
        store.consume_action(Action::LoadUsers);
        store.consume_action(Action::LoadIssueStatuses);
        store.consume_action(Action::LoadPriorities);
        store.consume_action(Action::LoadProjects);
        store.consume_action(Action::LoadTrackers);
        store.consume_action(Action::LoadTargetVersions);
        store.consume_action(Action::LoadComponents);
        store.consume_action(Action::LoadIssue { id: ISSUE_ID });
        store
    }

    fn assert_layout_contract(component: &PropertyComponent, store: &Store, cursor: Position) {
        assert_eq!(component.line_count(store, WIDTH), PROPERTY_LINE_COUNT);
        assert_eq!(component.get_cursor_position(), cursor);
    }

    #[test]
    fn focus_event_from_above_is_reflected_in_widget_and_cursor() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);

        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        assert_layout_contract(&component, &store, Position::new(20, 0));
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

        assert_layout_contract(&component, &store, Position::new(20, FOCUSABLE_LAST_LINE));
        render_snapshot(
            "property_component_focus_from_below",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_j_updates_widget_focus_and_cursor() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_layout_contract(&component, &store, Position::new(20, 1));
        render_snapshot(
            "property_component_process_j",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_e_returns_status_popup_result_without_changing_widget_focus() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..3 {
            component.process_event(key_event(KeyCode::Char('j')));
        }

        let result = component.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenIssueStatusPopup)
        ));
        assert_layout_contract(&component, &store, Position::new(20, 3));
        render_snapshot(
            "property_component_process_e_on_status",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_e_returns_assigned_to_popup_result_without_changing_widget_focus() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..7 {
            component.process_event(key_event(KeyCode::Char('j')));
        }

        let result = component.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenAssignedToPopup)
        ));
        assert_layout_contract(&component, &store, Position::new(20, 7));
    }

    #[test]
    fn process_event_e_returns_target_version_popup_result_without_changing_widget_focus() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..TARGET_VERSION_LINE {
            component.process_event(key_event(KeyCode::Char('j')));
        }

        let result = component.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenTargetVersionPopup)
        ));
        assert_layout_contract(&component, &store, Position::new(20, TARGET_VERSION_LINE));
    }

    #[test]
    fn process_event_e_returns_done_ratio_popup_result_without_changing_widget_focus() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromAbove);
        for _ in 0..DONE_RATIO_LINE {
            component.process_event(key_event(KeyCode::Char('j')));
        }

        let result = component.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenDoneRatioPopup)
        ));
        assert_layout_contract(&component, &store, Position::new(20, DONE_RATIO_LINE));
    }

    #[test]
    fn process_event_e_returns_spent_time_popup_result_without_changing_widget_focus() {
        let store = store_with_property_issue();
        let mut component = PropertyComponent::new(ISSUE_ID);
        component.focus_event(FocusEvent::CursorEnteredFromBelow);
        component.process_event(key_event(KeyCode::Char('k')));

        let result = component.process_event(key_event(KeyCode::Char('e')));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenSpentTimeInputPopup)
        ));
        assert_layout_contract(
            &component,
            &store,
            Position::new(20, TOTAL_SPENT_HOURS_LINE),
        );
        render_snapshot(
            "property_component_process_e_on_spent_time",
            WIDTH,
            component.line_count(&store, WIDTH),
            component.create_widget(&store),
        );
    }
}
