use std::collections::HashMap;
use std::num::NonZeroUsize;

use crossterm::event::Event;
use ratatui::layout::Rect;

use crate::stores::{ProjectIssuesPageState, Store};
use crate::vos::{IssueId, ProjectId};

use super::focus_state::{self, FocusState};
use super::widget::{
    IssueSelectPopupIssue, IssueSelectPopupProject, IssueSelectPopupWidget,
    IssueSelectPopupWidgetState,
};

pub enum EventProcessResult {
    Selected { issue_id: IssueId },
    Quited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    FetchProjectIssuesPage {
        project_id: ProjectId,
        page: NonZeroUsize,
    },
}

pub struct IssueSelectPopupComponent {
    projects: Vec<IssueSelectPopupProject>,
    display_pages: HashMap<ProjectId, NonZeroUsize>,
    pending_effect: Option<Effect>,
    focus_state: FocusState,
    widget_state: IssueSelectPopupWidgetState,
}

impl IssueSelectPopupComponent {
    /// focused_issue_idがある場合は、ポップアップを開いた時点で表示していたissueに
    /// フォーカスを合わせて初期化する。
    /// project一覧は生成時のsnapshotとし、現在pageのIssue表示はStoreから都度構成する。
    pub fn new(store: &Store, focused_issue_id: Option<IssueId>) -> Self {
        let mut projects = store
            .get_projects()
            .iter()
            .map(|(id, project)| IssueSelectPopupProject::new(*id, project.name.clone()))
            .collect::<Vec<_>>();
        projects.sort_by_key(|project| project.id);
        let focused_issues_project =
            |project_id: &ProjectId| projects.iter().any(|project| project.id == *project_id);
        let focused_issues_project_id = focused_issue_id
            .and_then(|issue_id| store.get_issue(issue_id).map(|(issue, _)| issue.project_id))
            .filter(focused_issues_project);
        let focused_project_id =
            focused_issues_project_id.or_else(|| projects.first().map(|project| project.id));
        let mut component = Self {
            projects,
            display_pages: HashMap::new(),
            pending_effect: None,
            focus_state: FocusState::new(),
            widget_state: IssueSelectPopupWidgetState::new(),
        };
        component.refresh_focus_counts(store);
        if let Some(project_id) = focused_project_id {
            component.focus_project(project_id);
            component
                .display_pages
                .insert(project_id, NonZeroUsize::MIN);
            component.activate_focused_project(store);
            if let Some(focused_issue_id) = focused_issue_id {
                component.focus_issue_in_current_page(store, focused_issue_id);
            }
            component.install_fetch_effect(project_id, NonZeroUsize::MIN);
        }
        component
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, store: &Store, area: Rect) {
        let focused_issue_id = self.focused_issue_id(store);
        self.refresh_focus_counts(store);
        if let Some((project_id, page)) = self.focused_project_and_page()
            && page == NonZeroUsize::MIN
            && matches!(
                store.get_project_issues_page_state(project_id, page),
                Some(ProjectIssuesPageState::Loaded { issues, .. }) if issues.is_empty()
            )
        {
            self.focus_state.focus_project_column();
        }
        if let Some(issue_id) = focused_issue_id {
            self.focus_issue_in_current_page(store, issue_id);
        }

        let issues = self.current_page_issues(store);
        if let Some(issue) = issues.get(self.focus_state.focused_issue_index()) {
            self.widget_state.update(
                IssueSelectPopupWidget::preview_width(area),
                issue,
                &issue.description,
            );
        }
        self.widget_state.update_scroll(
            area,
            self.focus_state.focused_project_index(),
            self.focus_state.focused_issue_index(),
        );
    }

    pub fn process_event(&mut self, event: Event, store: &Store) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .and_then(|result| match result {
                focus_state::EventProcessResult::Selected => self
                    .focused_issue_id(store)
                    .map(|issue_id| EventProcessResult::Selected { issue_id }),
                focus_state::EventProcessResult::Quited => Some(EventProcessResult::Quited),
                focus_state::EventProcessResult::ProjectChanged => {
                    if let Some(project_id) = self.focused_project_id() {
                        let page = *self
                            .display_pages
                            .entry(project_id)
                            .or_insert(NonZeroUsize::MIN);
                        self.activate_focused_project(store);
                        self.request_page(project_id, page);
                    }
                    None
                }
                focus_state::EventProcessResult::PreviousPageRequested => {
                    if let Some((project_id, page)) = self.focused_project_and_page()
                        && matches!(
                            store.get_project_issues_page_state(project_id, page),
                            Some(ProjectIssuesPageState::Loaded { .. })
                        )
                        && let Some(previous) =
                            page.get().checked_sub(1).and_then(NonZeroUsize::new)
                    {
                        self.navigate_to_page(store, project_id, previous);
                    }
                    None
                }
                focus_state::EventProcessResult::NextPageRequested => {
                    if let Some((project_id, page)) = self.focused_project_and_page()
                        && let Some(ProjectIssuesPageState::Loaded {
                            issues,
                            total_count,
                            offset,
                            ..
                        }) = store.get_project_issues_page_state(project_id, page)
                        && !issues.is_empty()
                        && offset.saturating_add(issues.len()) < *total_count
                        && let Some(next) = page.get().checked_add(1).and_then(NonZeroUsize::new)
                    {
                        self.navigate_to_page(store, project_id, next);
                    }
                    None
                }
                focus_state::EventProcessResult::RetryRequested => {
                    if let Some((project_id, page)) = self.focused_project_and_page()
                        && matches!(
                            store.get_project_issues_page_state(project_id, page),
                            Some(ProjectIssuesPageState::Failed { .. })
                        )
                    {
                        self.request_page(project_id, page);
                    }
                    None
                }
            })
    }

    pub fn take_effect(&mut self) -> Option<Effect> {
        self.pending_effect.take()
    }

    pub fn create_widget<'a>(&'a self, store: &'a Store) -> IssueSelectPopupWidget<'a> {
        let issues = self.current_page_issues(store);
        let column_state = self
            .focused_project_and_page()
            .and_then(|(project_id, page)| store.get_project_issues_page_state(project_id, page));
        IssueSelectPopupWidget::new(
            &self.projects,
            &issues,
            self.focus_state.focused_project_index(),
            self.focus_state.focused_issue_index(),
            self.focus_state.focused_column(),
            &self.widget_state,
            match column_state {
                Some(ProjectIssuesPageState::Loading { .. }) => {
                    super::widget::IssueSelectPopupIssueColumnState::Loading
                }
                Some(ProjectIssuesPageState::Failed { message }) => {
                    super::widget::IssueSelectPopupIssueColumnState::Failed { message }
                }
                Some(ProjectIssuesPageState::Loaded { issues, .. }) if issues.is_empty() => {
                    super::widget::IssueSelectPopupIssueColumnState::LoadedEmpty
                }
                Some(ProjectIssuesPageState::Loaded { .. }) => {
                    super::widget::IssueSelectPopupIssueColumnState::Loaded
                }
                _ => super::widget::IssueSelectPopupIssueColumnState::Unloaded,
            },
        )
    }

    fn current_page_issues(&self, store: &Store) -> Vec<IssueSelectPopupIssue> {
        self.focused_project_and_page()
            .and_then(|(project_id, page)| store.get_project_issues(project_id, page))
            .unwrap_or_default()
            .iter()
            .map(|issue| {
                if let Some((loaded, _)) = store.get_issue(issue.id) {
                    IssueSelectPopupIssue::new(
                        issue.project_id,
                        issue.id,
                        loaded.subject.clone(),
                        loaded.description.clone(),
                    )
                } else {
                    IssueSelectPopupIssue::new(
                        issue.project_id,
                        issue.id,
                        issue.subject.clone(),
                        issue.description.clone(),
                    )
                }
            })
            .collect()
    }

    fn project_issue_counts(&self, store: &Store) -> Vec<usize> {
        self.projects
            .iter()
            .map(|project| {
                self.display_pages
                    .get(&project.id)
                    .and_then(|page| store.get_project_issues(project.id, *page))
                    .map_or(0, |issues| issues.len())
            })
            .collect()
    }

    fn focus_issue_in_current_page(&mut self, store: &Store, issue_id: impl Into<IssueId>) {
        let issue_id = issue_id.into();
        let Some(issue_index) = self
            .focused_project_and_page()
            .and_then(|(project_id, page)| store.get_project_issues(project_id, page))
            .and_then(|issues| issues.iter().position(|issue| issue.id == issue_id))
        else {
            return;
        };
        self.focus_state
            .focus_issue(self.focus_state.focused_project_index(), issue_index);
    }

    fn focused_issue_id(&self, store: &Store) -> Option<IssueId> {
        self.focused_project_and_page()
            .and_then(|(project_id, page)| store.get_project_issues(project_id, page))
            .and_then(|issues| issues.get(self.focus_state.focused_issue_index()))
            .map(|issue| issue.id)
    }

    fn focus_project(&mut self, project_id: ProjectId) {
        if let Some(project_index) = self
            .projects
            .iter()
            .position(|project| project.id == project_id)
        {
            self.focus_state.focus_issue(project_index, 0);
        }
    }

    fn focused_project_id(&self) -> Option<ProjectId> {
        self.projects
            .get(self.focus_state.focused_project_index())
            .map(|project| project.id)
    }

    fn focused_project_and_page(&self) -> Option<(ProjectId, NonZeroUsize)> {
        let project_id = self.focused_project_id()?;
        self.display_pages
            .get(&project_id)
            .copied()
            .map(|page| (project_id, page))
    }

    fn refresh_focus_counts(&mut self, store: &Store) {
        self.focus_state
            .replace_project_issue_counts(self.project_issue_counts(store));
        let empty_issue_column_enterable =
            self.focused_project_and_page()
                .is_some_and(|(project_id, page)| {
                    page != NonZeroUsize::MIN
                        && matches!(
                            store.get_project_issues_page_state(project_id, page),
                            Some(ProjectIssuesPageState::Loaded { issues, .. }) if issues.is_empty()
                        )
                });
        self.focus_state
            .set_empty_issue_column_enterable(empty_issue_column_enterable);
    }

    fn activate_focused_project(&mut self, store: &Store) {
        self.focus_state.reset_issue_focus();
        self.refresh_focus_counts(store);
    }

    fn navigate_to_page(&mut self, store: &Store, project_id: ProjectId, page: NonZeroUsize) {
        self.display_pages.insert(project_id, page);
        self.activate_focused_project(store);
        self.request_page(project_id, page);
    }

    fn request_page(&mut self, project_id: ProjectId, page: NonZeroUsize) {
        self.display_pages.insert(project_id, page);
        self.focus_state.reset_issue_focus();
        self.install_fetch_effect(project_id, page);
    }

    fn install_fetch_effect(&mut self, project_id: ProjectId, page: NonZeroUsize) {
        assert!(
            self.pending_effect.is_none(),
            "IssueSelectPopupComponent already has a pending effect"
        );
        self.pending_effect = Some(Effect::FetchProjectIssuesPage { project_id, page });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use crate::components::issue_select_popup::widget::IssueSelectPopupFocusColumn;
    use crate::entities::{ProjectIssuesPage, ProjectsIssue};
    use crate::stores::IssueAction;
    use crate::stores::ProjectIssuesAction;
    use crate::test_support::{render_snapshot, sample_issue_aggregate, sync_fixture_entities};

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn unloaded_store() -> Store {
        let mut store = Store::new();
        sync_fixture_entities(&mut store);
        store.consume_action(IssueAction::Load { id: 1.into() }.into());
        store.consume_action(IssueAction::Load { id: 2.into() }.into());
        store.consume_action(IssueAction::Load { id: 3.into() }.into());
        store
    }

    fn store() -> Store {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![
                project_issue(1, 1, "issue1", "body"),
                project_issue(2, 1, "issue2", "body"),
                project_issue(3, 1, "issue3", "body"),
            ],
            3,
            0,
        );
        store
    }

    fn page(number: usize) -> std::num::NonZeroUsize {
        std::num::NonZeroUsize::new(number).unwrap()
    }

    fn load_project_page(
        store: &mut Store,
        project_id: u16,
        page_number: usize,
        issues: Vec<ProjectsIssue>,
        total_count: usize,
        offset: usize,
    ) {
        let request_id = crate::stores::ProjectIssuesRequestId::new();
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id,
                project_id: project_id.into(),
                page: page(page_number),
            }
            .into(),
        );
        store.consume_action(
            ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id: project_id.into(),
                page: page(page_number),
                result: ProjectIssuesPage {
                    issues,
                    total_count,
                    offset,
                    limit: 50,
                },
            }
            .into(),
        );
    }

    fn project_issue(id: u16, project_id: u16, subject: &str, description: &str) -> ProjectsIssue {
        ProjectsIssue {
            id: id.into(),
            project_id: project_id.into(),
            subject: subject.to_string(),
            description: description.to_string(),
            status_id: 1.into(),
        }
    }

    #[test]
    fn new_requests_first_page_of_only_the_initially_selected_project_once() {
        let store = unloaded_store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(3.into()));

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page })
                if project_id == 1
                    && requested_page == page(1)
        ));
        assert!(component.take_effect().is_none());
    }

    #[test]
    fn new_requests_page_one_even_when_the_exact_page_is_already_loaded() {
        let store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(1)
        ));
        assert!(component.take_effect().is_none());
    }

    #[test]
    fn new_with_no_projects_has_no_initial_effect() {
        let mut component = IssueSelectPopupComponent::new(&Store::new(), None);

        assert!(component.take_effect().is_none());
    }

    #[test]
    fn loaded_projects_issues_are_displayed_and_loaded_issue_values_take_precedence() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![
                project_issue(1, 1, "server list subject", "server list body"),
                project_issue(42, 1, "unloaded subject", "unloaded body"),
            ],
            2,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.update(&store, AREA);

        let widget = component.create_widget(&store);
        assert_eq!(widget.issues.len(), 2);
        assert_eq!(
            widget.issues[0].subject,
            store.get_issue(1).unwrap().0.subject
        );
        assert_eq!(
            widget.issues[0].description,
            store.get_issue(1).unwrap().0.description
        );
        assert_eq!(widget.issues[1].subject, "unloaded subject");
        assert_eq!(widget.issues[1].description, "unloaded body");
    }

    #[test]
    fn projects_issue_does_not_determine_the_initial_project_when_issue_store_lacks_the_issue() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(10, 1, "project one", "body")],
            1,
            0,
        );
        load_project_page(
            &mut store,
            2,
            3,
            vec![project_issue(42, 2, "project two", "body")],
            1,
            100,
        );
        assert!(store.get_issue(42).is_none());

        let mut component = IssueSelectPopupComponent::new(&store, Some(42.into()));
        let widget = component.create_widget(&store);

        assert_eq!(widget.focused_project_index, 0);
        assert_eq!(widget.issues[0].issue_id, IssueId::new(10));
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(1)
        ));
    }

    #[test]
    fn issue_store_project_outside_the_popup_snapshot_falls_back_to_the_first_project() {
        let mut store = unloaded_store();
        let mut issue = sample_issue_aggregate(42, "detail", 1.into(), None, None, None, 0);
        issue.project_id = 99.into();
        store.consume_action(IssueAction::Sync { issue }.into());
        load_project_page(
            &mut store,
            2,
            3,
            vec![project_issue(42, 2, "project two", "body")],
            1,
            100,
        );

        let mut component = IssueSelectPopupComponent::new(&store, Some(42.into()));

        assert_eq!(component.create_widget(&store).focused_project_index, 0);
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(1)
        ));
    }

    #[test]
    fn issue_store_project_in_the_popup_snapshot_sets_the_initial_project() {
        let mut store = unloaded_store();
        let issue = sample_issue_aggregate(42, "detail", 1.into(), None, None, None, 0);
        store.consume_action(IssueAction::Sync { issue }.into());
        load_project_page(
            &mut store,
            2,
            3,
            vec![project_issue(42, 2, "project two", "body")],
            1,
            100,
        );

        let mut component = IssueSelectPopupComponent::new(&store, Some(42.into()));

        assert_eq!(component.create_widget(&store).focused_project_index, 0);
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(1)
        ));
    }

    #[test]
    fn boundary_navigation_and_failed_retry_request_the_store_selected_page() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(42, 1, "subject", "body")],
            51,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);

        component.process_event(key_event(KeyCode::Char('j')), &store);
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { page: requested_page, .. }) if requested_page == page(2)
        ));

        let request_id = crate::stores::ProjectIssuesRequestId::new();
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id,
                project_id: 1.into(),
                page: page(2),
            }
            .into(),
        );
        store.consume_action(
            ProjectIssuesAction::LoadFailed {
                request_id,
                project_id: 1.into(),
                page: page(2),
                message: "offline".to_string(),
            }
            .into(),
        );
        component.update(&store, AREA);
        component.process_event(key_event(KeyCode::Char('r')), &store);
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { page: requested_page, .. }) if requested_page == page(2)
        ));
    }

    #[test]
    fn failed_page_can_be_retried_from_the_project_column() {
        let mut store = unloaded_store();
        let request_id = crate::stores::ProjectIssuesRequestId::new();
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id,
                project_id: 1.into(),
                page: page(1),
            }
            .into(),
        );
        store.consume_action(
            ProjectIssuesAction::LoadFailed {
                request_id,
                project_id: 1.into(),
                page: page(1),
                message: "offline".to_string(),
            }
            .into(),
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();

        component.process_event(key_event(KeyCode::Char('r')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page })
                if project_id == 1 && requested_page == page(1)
        ));
    }

    #[test]
    fn page_navigation_requests_an_already_loaded_destination_again() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "first page", "body")],
            51,
            0,
        );
        load_project_page(
            &mut store,
            1,
            2,
            vec![project_issue(42, 1, "cached second page", "body")],
            51,
            50,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);

        component.process_event(key_event(KeyCode::Char('j')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { page: requested_page, .. })
                if requested_page == page(2)
        ));
    }

    #[test]
    fn next_boundary_refetches_a_loading_destination() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "first page", "body")],
            51,
            0,
        );
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id: crate::stores::ProjectIssuesRequestId::new(),
                project_id: 1.into(),
                page: page(2),
            }
            .into(),
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);

        component.process_event(key_event(KeyCode::Char('j')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page })
                if project_id == 1 && requested_page == page(2)
        ));
        assert!(matches!(
            component.create_widget(&store).issue_column_state,
            super::super::widget::IssueSelectPopupIssueColumnState::Loading
        ));
    }

    #[test]
    fn next_boundary_uses_only_the_current_exact_pages_metadata() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "terminal first page", "body")],
            1,
            0,
        );
        load_project_page(
            &mut store,
            1,
            2,
            vec![project_issue(42, 1, "nonterminal cached page", "body")],
            101,
            50,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);

        component.process_event(key_event(KeyCode::Char('j')), &store);

        assert!(component.take_effect().is_none());
        assert_eq!(component.create_widget(&store).issues[0].issue_id, 1);
    }

    #[test]
    fn first_move_to_an_unloaded_project_requests_only_its_first_page() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "subject", "body")],
            1,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(1)
        ));

        component.process_event(key_event(KeyCode::Char('j')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 2 && requested_page == page(1)
        ));
        assert!(component.take_effect().is_none());
    }

    #[test]
    fn switching_projects_restores_each_projects_last_page_and_refetches_a_b_a() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "project one", "body")],
            51,
            0,
        );
        load_project_page(
            &mut store,
            1,
            2,
            vec![project_issue(11, 1, "project one page two", "body")],
            51,
            50,
        );
        load_project_page(
            &mut store,
            2,
            1,
            vec![project_issue(42, 2, "project two", "body")],
            1,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        let _ = component.take_effect();

        component.process_event(key_event(KeyCode::Char('h')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        assert_eq!(component.create_widget(&store).issues[0].issue_id, 42);
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page })
                if project_id == 2 && requested_page == page(1)
        ));

        component.process_event(key_event(KeyCode::Char('k')), &store);
        assert_eq!(component.create_widget(&store).issues[0].issue_id, 11);
        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page })
                if project_id == 1 && requested_page == page(2)
        ));
    }

    #[test]
    fn switching_back_refetches_even_when_the_exact_page_is_loading() {
        let mut store = unloaded_store();
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id: crate::stores::ProjectIssuesRequestId::new(),
                project_id: 1.into(),
                page: page(1),
            }
            .into(),
        );
        load_project_page(
            &mut store,
            2,
            1,
            vec![project_issue(42, 2, "project two", "body")],
            1,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();

        component.process_event(key_event(KeyCode::Char('j')), &store);
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('k')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page })
                if project_id == 1 && requested_page == page(1)
        ));
    }

    #[test]
    fn switching_back_refetches_the_remembered_page() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "project one", "body")],
            51,
            0,
        );
        load_project_page(
            &mut store,
            1,
            2,
            vec![project_issue(11, 1, "project one page two", "body")],
            51,
            50,
        );
        load_project_page(
            &mut store,
            2,
            1,
            vec![project_issue(42, 2, "project two", "body")],
            1,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('h')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        let _ = component.take_effect();

        component.process_event(key_event(KeyCode::Char('k')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(2)
        ));
    }

    #[test]
    fn loading_project_blocks_issue_focus_selection_and_retry() {
        let mut store = unloaded_store();
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id: crate::stores::ProjectIssuesRequestId::new(),
                project_id: 1.into(),
                page: page(1),
            }
            .into(),
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { project_id, page: requested_page, .. })
                if project_id == 1 && requested_page == page(1)
        ));
        component.process_event(key_event(KeyCode::Char('l')), &store);
        assert_eq!(
            component.create_widget(&store).focused_column,
            IssueSelectPopupFocusColumn::Project
        );
        assert!(
            component
                .process_event(key_event(KeyCode::Enter), &store)
                .is_none()
        );
        component.process_event(key_event(KeyCode::Char('r')), &store);
        assert!(component.take_effect().is_none());
    }

    #[test]
    fn empty_second_page_can_reenter_issue_column_and_k_requests_previous_page() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "subject", "body")],
            51,
            0,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        let _ = component.take_effect();
        let request_id = crate::stores::ProjectIssuesRequestId::new();
        store.consume_action(
            ProjectIssuesAction::StartLoading {
                request_id,
                project_id: 1.into(),
                page: page(2),
            }
            .into(),
        );
        store.consume_action(
            ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id: 1.into(),
                page: page(2),
                result: ProjectIssuesPage {
                    issues: Vec::new(),
                    total_count: 50,
                    offset: 50,
                    limit: 50,
                },
            }
            .into(),
        );
        component.update(&store, AREA);

        component.process_event(key_event(KeyCode::Char('h')), &store);
        component.process_event(key_event(KeyCode::Char('l')), &store);
        assert_eq!(
            component.create_widget(&store).focused_column,
            IssueSelectPopupFocusColumn::Issue
        );
        component.process_event(key_event(KeyCode::Char('k')), &store);

        assert!(matches!(
            component.take_effect(),
            Some(Effect::FetchProjectIssuesPage { page: requested_page, .. }) if requested_page == page(1)
        ));
    }

    #[test]
    fn loaded_empty_first_page_returns_focus_to_the_project_column() {
        let mut store = unloaded_store();
        load_project_page(
            &mut store,
            1,
            1,
            vec![project_issue(1, 1, "first page", "body")],
            51,
            0,
        );
        load_project_page(
            &mut store,
            1,
            2,
            vec![project_issue(42, 1, "second page", "body")],
            51,
            50,
        );
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));
        let _ = component.take_effect();
        component.process_event(key_event(KeyCode::Char('l')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        let _ = component.take_effect();

        load_project_page(&mut store, 1, 1, Vec::new(), 0, 0);

        component.process_event(key_event(KeyCode::Char('k')), &store);
        let _ = component.take_effect();
        component.update(&store, AREA);

        assert_eq!(
            component.create_widget(&store).focused_column,
            IssueSelectPopupFocusColumn::Project
        );
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
        let _ = component.take_effect();

        component.process_event(key_event(KeyCode::Char('j')), &store);
        component.process_event(key_event(KeyCode::Char('l')), &store);
        let widget = component.create_widget(&store);

        assert_eq!(widget.focused_project_index, 1);
        assert_eq!(widget.focused_column, IssueSelectPopupFocusColumn::Project);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        let result = component.process_event(key_event(KeyCode::Char('q')), &store);

        assert!(matches!(result, Some(EventProcessResult::Quited)));
    }

    #[test]
    fn process_event_enter_returns_selected_issue_id() {
        let store = store();
        let mut component = IssueSelectPopupComponent::new(&store, Some(1.into()));

        component.process_event(key_event(KeyCode::Char('l')), &store);
        component.process_event(key_event(KeyCode::Char('j')), &store);
        let result = component.process_event(key_event(KeyCode::Enter), &store);

        match result {
            Some(EventProcessResult::Selected { issue_id }) => {
                assert_eq!(issue_id, IssueId::new(2));
            }
            _ => panic!("expected selected issue"),
        }
    }
}
