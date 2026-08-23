use crossterm::event::Event;
use ratatui::layout::Rect;

use crate::app::Store;
use crate::vos::{EntityIdValue, IssueId};

use super::focus_state::{self, FocusState};
use super::widget::{
    IssueSelectPopupIssue, IssueSelectPopupProject, IssueSelectPopupWidget,
    IssueSelectPopupWidgetState,
};

pub enum EventProcessResult {
    Selected { issue_id: IssueId },
    Quited,
}

pub struct IssueSelectPopupComponent {
    projects: Vec<IssueSelectPopupProject>,
    issues: Vec<IssueSelectPopupIssue>,
    focus_state: FocusState,
    widget_state: IssueSelectPopupWidgetState,
}

impl IssueSelectPopupComponent {
    /// focused_issue_idがある場合は、ポップアップを開いた時点で表示していたissueに
    /// フォーカスを合わせて初期化する。
    /// projects/issuesの一覧はupdateでStoreから取得する。
    pub fn new(store: &Store, focused_issue_id: Option<IssueId>) -> Self {
        let mut component = Self {
            projects: Vec::new(),
            issues: Vec::new(),
            focus_state: FocusState::new(),
            widget_state: IssueSelectPopupWidgetState::new(),
        };
        component.load_from_store(store);
        if let Some(focused_issue_id) = focused_issue_id {
            component.focus_issue(focused_issue_id);
        }
        component
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, store: &Store, area: Rect) {
        let focused_issue_id = self.focused_issue_id();

        self.load_from_store(store);

        // Storeの更新で一覧の並びが変わってもフォーカス中のissueを維持する。
        if let Some(issue_id) = focused_issue_id {
            self.focus_issue(issue_id);
        }

        if let Some(issue) = self.focused_issue().cloned() {
            let description = &store
                .get_issues()
                .get(&issue.issue_id)
                .expect("IssueSelectPopupCopmonent requires its issue to exist in Store")
                .description;
            self.widget_state.update(
                IssueSelectPopupWidget::preview_width(area),
                &issue,
                description,
            );
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .and_then(|result| match result {
                focus_state::EventProcessResult::Selected => self
                    .focused_issue_id()
                    .map(|issue_id| EventProcessResult::Selected { issue_id }),
                focus_state::EventProcessResult::Quited => Some(EventProcessResult::Quited),
            })
    }

    pub fn create_widget<'a>(&'a self, store: &'a Store) -> IssueSelectPopupWidget<'a> {
        let visible_issues = self.visible_issues();
        let empty_description = String::new();
        let description = self
            .focused_issue_id()
            .map(|issue_id| {
                &store
                    .get_issues()
                    .get(&issue_id)
                    .expect("IssueSelectPopupComponent requires its issue to exist in Store")
                    .description
            })
            .unwrap_or(&empty_description);
        IssueSelectPopupWidget::new(
            &self.projects,
            visible_issues,
            self.focus_state.focused_project_index(),
            self.focus_state.focused_issue_index(),
            self.focus_state.focused_column(),
            &self.widget_state,
            description,
        )
    }

    fn load_from_store(&mut self, store: &Store) {
        // FIXME: 持っているIssueの一覧ではなく、ProjectのGETに含まれるIssue
        // subjectの一覧を使用する
        self.issues = {
            let mut issues = store
                .get_issues()
                .iter()
                .map(|(id, issue)| {
                    IssueSelectPopupIssue::new(issue.project_id.get(), *id, issue.subject.clone())
                })
                .collect::<Vec<_>>();
            issues.sort_by_key(|issue| issue.issue_id);
            issues
        };

        self.projects = {
            let mut projects = store
                .get_projects()
                .iter()
                .map(|(id, project)| IssueSelectPopupProject::new(*id, project.name.clone()))
                .collect::<Vec<_>>();
            projects.sort_by_key(|project| project.id);
            projects
        };

        self.focus_state
            .replace_project_issue_counts(self.project_issue_counts());
    }

    fn project_issue_counts(&self) -> Vec<usize> {
        self.projects
            .iter()
            .map(|project| {
                self.issues
                    .iter()
                    .filter(|issue| issue.project_id == project.id)
                    .count()
            })
            .collect()
    }

    /// issue_idを持つissueとその所属projectにフォーカスを合わせる。
    /// 見つからない場合は現在のフォーカスを一覧の範囲内に丸める。
    fn focus_issue(&mut self, issue_id: impl Into<IssueId>) {
        let issue_id = issue_id.into();
        let Some(issue) = self.issues.iter().find(|issue| issue.issue_id == issue_id) else {
            self.focus_state
                .replace_project_issue_counts(self.project_issue_counts());
            return;
        };

        let project_id = issue.project_id;
        let Some(project_index) = self
            .projects
            .iter()
            .position(|project| project.id == project_id)
        else {
            self.focus_state
                .replace_project_issue_counts(self.project_issue_counts());
            return;
        };

        let issue_index = self
            .issues
            .iter()
            .filter(|issue| issue.project_id == project_id)
            .position(|issue| issue.issue_id == issue_id)
            .unwrap_or(0);

        self.focus_state.focus_issue(project_index, issue_index);
    }

    fn focused_issue(&self) -> Option<&IssueSelectPopupIssue> {
        self.visible_issues()
            .get(self.focus_state.focused_issue_index())
            .copied()
    }

    fn visible_issues(&self) -> Vec<&IssueSelectPopupIssue> {
        let Some(project) = self.projects.get(self.focus_state.focused_project_index()) else {
            return Vec::new();
        };

        self.issues
            .iter()
            .filter(|issue| issue.project_id == project.id)
            .collect()
    }

    fn focused_issue_id(&self) -> Option<IssueId> {
        self.focused_issue().map(|issue| issue.issue_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use crate::app::Action;
    use crate::components::issue_select_popup::widget::IssueSelectPopupFocusColumn;
    use crate::test_support::{render_snapshot, sync_fixture_entities};

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn store() -> Store {
        let mut store = Store::new();
        sync_fixture_entities(&mut store);
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::LoadIssue { id: 2.into() });
        store.consume_action(Action::LoadIssue { id: 3.into() });
        store
    }

    #[test]
    fn new_without_focused_issue_handles_empty_store() {
        let store = Store::new();
        let mut component = IssueSelectPopupComponent::new(&store, None);

        component.update(&store, AREA);
        let widget = component.create_widget(&store);

        assert!(widget.projects.is_empty());
        assert!(widget.issues.is_empty());
    }

    #[test]
    fn snapshot_update_renders_initial_focus_and_preview() {
        let store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(2.into()));

        component.update(&store, AREA);

        render_snapshot(
            "issue_select_popup_component_initial_focus",
            AREA.width,
            AREA.height,
            component.create_widget(&store),
        );
    }

    #[test]
    fn snapshot_process_event_updates_focus_state_visible_in_widget() {
        let store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        component.process_event(key_event(KeyCode::Char('l')));
        component.process_event(key_event(KeyCode::Char('j')));
        component.update(&store, AREA);

        render_snapshot(
            "issue_select_popup_component_process_event_focus",
            AREA.width,
            AREA.height,
            component.create_widget(&store),
        );
    }

    #[test]
    fn snapshot_update_reflects_issue_body_from_store() {
        let mut store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "updated description".to_string(),
        });
        component.update(&store, AREA);

        render_snapshot(
            "issue_select_popup_component_updated_body",
            AREA.width,
            AREA.height,
            component.create_widget(&store),
        );
    }

    #[test]
    fn create_widget_includes_projects_with_no_issues() {
        let store = store();
        let component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let widget = component.create_widget(&store);

        assert_eq!(widget.projects.len(), 2);
        assert_eq!(widget.projects[0].name, "Sample Project");
        assert_eq!(widget.projects[1].name, "Sample Project 2");
    }

    #[test]
    fn process_event_l_ignores_empty_project_focus() {
        let store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('l')));
        let widget = component.create_widget(&store);

        assert_eq!(widget.focused_project_index, 1);
        assert_eq!(widget.focused_column, IssueSelectPopupFocusColumn::Project);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let mut component = IssueSelectPopupComponent::new(&store(), Some(1.into()));

        let result = component.process_event(key_event(KeyCode::Char('q')));

        assert!(matches!(result, Some(EventProcessResult::Quited)));
    }

    #[test]
    fn process_event_enter_returns_selected_issue_id() {
        let mut component = IssueSelectPopupComponent::new(&store(), Some(1.into()));

        component.process_event(key_event(KeyCode::Char('l')));
        component.process_event(key_event(KeyCode::Char('j')));
        let result = component.process_event(key_event(KeyCode::Enter));

        match result {
            Some(EventProcessResult::Selected { issue_id }) => {
                assert_eq!(issue_id, IssueId::new(2));
            }
            _ => panic!("expected selected issue"),
        }
    }

    #[test]
    fn process_event_returns_none_for_non_key_event() {
        let mut component = IssueSelectPopupComponent::new(&store(), Some(1.into()));

        let result = component.process_event(Event::Resize(80, 24));

        assert!(result.is_none());
    }

    #[test]
    fn process_event_ignores_unhandled_key() {
        let mut component = IssueSelectPopupComponent::new(&store(), Some(2.into()));

        let result = component.process_event(key_event(KeyCode::Char('x')));

        assert!(result.is_none());
    }
}
