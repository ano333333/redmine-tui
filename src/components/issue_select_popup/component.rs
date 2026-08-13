use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;

use crate::app::Store;
use crate::vos::{EntityIdValue, IssueId, ProjectId};

use super::widget::{
    IssueSelectPopupFocusColumn, IssueSelectPopupIssue, IssueSelectPopupProject,
    IssueSelectPopupWidget, IssueSelectPopupWidgetState,
};

pub struct IssueSelectPopupComponent {
    projects: Vec<IssueSelectPopupProject>,
    issues: Vec<IssueSelectPopupIssue>,
    focused_project_index: usize,
    focused_issue_index: usize,
    focused_column: IssueSelectPopupFocusColumn,
    widget_state: IssueSelectPopupWidgetState,
}

impl IssueSelectPopupComponent {
    /// ポップアップを開いた時点で表示していたissueにフォーカスを合わせて初期化する。
    /// projects/issuesの一覧はupdateでStoreから取得する。
    pub fn new(store: &Store, focused_issue_id: impl Into<IssueId>) -> Self {
        let mut component = Self {
            projects: Vec::new(),
            issues: Vec::new(),
            focused_project_index: 0,
            focused_issue_index: 0,
            focused_column: IssueSelectPopupFocusColumn::Project,
            widget_state: IssueSelectPopupWidgetState::new(),
        };
        component.load_from_store(store);
        component.focus_issue(focused_issue_id);
        component
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, store: &Store, area: Rect) {
        let focused_issue_id = self.focused_issue_id();

        self.load_from_store(store);

        // Storeの更新で一覧の並びが変わってもフォーカス中のissueを維持する
        match focused_issue_id {
            Some(issue_id) => self.focus_issue(issue_id),
            None => self.clamp_focus(),
        }

        if let Some(issue) = self.focused_issue().cloned() {
            self.widget_state
                .update(IssueSelectPopupWidget::preview_width(area), &issue);
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => match self.focused_column {
                IssueSelectPopupFocusColumn::Project => self.focus_next_project(),
                IssueSelectPopupFocusColumn::Issue => self.focus_next_issue(),
            },
            KeyCode::Char('k') => match self.focused_column {
                IssueSelectPopupFocusColumn::Project => self.focus_previous_project(),
                IssueSelectPopupFocusColumn::Issue => self.focus_previous_issue(),
            },
            KeyCode::Char('h') => {
                self.focused_column = IssueSelectPopupFocusColumn::Project;
            }
            KeyCode::Char('l') => {
                self.focused_column = IssueSelectPopupFocusColumn::Issue;
            }
            KeyCode::Char('q') => {
                return Some(EventProcessResult::Quited);
            }
            KeyCode::Enter => {
                if let Some(issue_id) = self.focused_issue_id() {
                    return Some(EventProcessResult::Entered { issue_id });
                }
            }
            _ => {}
        }

        None
    }

    pub fn create_widget<'a>(&'a self) -> IssueSelectPopupWidget<'a> {
        let visible_issues = self.visible_issues();
        IssueSelectPopupWidget::new(
            &self.projects,
            visible_issues,
            self.focused_project_index,
            self.focused_issue_index,
            self.focused_column,
            &self.widget_state,
        )
    }

    fn load_from_store(&mut self, store: &Store) {
        self.issues = {
            let mut issues = store
                .get_issues()
                .iter()
                .map(|(id, issue)| {
                    IssueSelectPopupIssue::new(
                        issue.project_id.get(),
                        *id,
                        issue.subject.clone(),
                        issue.description.clone(),
                    )
                })
                .collect::<Vec<_>>();
            issues.sort_by_key(|issue| issue.issue_id);
            issues
        };

        self.projects = {
            let mut projects = store
                .get_projects()
                .iter()
                .filter(|(id, _)| {
                    self.issues
                        .iter()
                        .any(|issue| issue.project_id == ProjectId::new(**id))
                })
                .map(|(id, project)| IssueSelectPopupProject::new(*id, project.name.clone()))
                .collect::<Vec<_>>();
            projects.sort_by_key(|project| project.id);
            projects
        };
    }

    /// issue_idを持つissueとその所属projectにフォーカスを合わせる。
    /// 見つからない場合は現在のフォーカスを一覧の範囲内に丸める。
    fn focus_issue(&mut self, issue_id: impl Into<IssueId>) {
        let issue_id = issue_id.into();
        let Some(issue) = self.issues.iter().find(|issue| issue.issue_id == issue_id) else {
            self.clamp_focus();
            return;
        };

        let project_id = issue.project_id;
        let Some(project_index) = self
            .projects
            .iter()
            .position(|project| project.id == project_id)
        else {
            self.clamp_focus();
            return;
        };

        self.focused_project_index = project_index;
        self.focused_issue_index = self
            .issues
            .iter()
            .filter(|issue| issue.project_id == project_id)
            .position(|issue| issue.issue_id == issue_id)
            .unwrap_or(0);
    }

    fn clamp_focus(&mut self) {
        self.focused_project_index = self
            .focused_project_index
            .min(self.projects.len().saturating_sub(1));
        self.focused_issue_index = self
            .focused_issue_index
            .min(self.focused_project_issue_count().saturating_sub(1));
    }

    fn focus_next_project(&mut self) {
        if self.focused_project_index + 1 < self.projects.len() {
            self.focused_project_index += 1;
            self.focused_issue_index = 0;
        }
    }

    fn focus_previous_project(&mut self) {
        if self.focused_project_index > 0 {
            self.focused_project_index -= 1;
            self.focused_issue_index = 0;
        }
    }

    fn focus_next_issue(&mut self) {
        if self.focused_issue_index + 1 < self.focused_project_issue_count() {
            self.focused_issue_index += 1;
        }
    }

    fn focus_previous_issue(&mut self) {
        if self.focused_issue_index > 0 {
            self.focused_issue_index -= 1;
        }
    }

    fn focused_project_issue_count(&self) -> usize {
        self.visible_issues().len()
    }

    fn focused_issue(&self) -> Option<&IssueSelectPopupIssue> {
        self.visible_issues().get(self.focused_issue_index).copied()
    }

    fn visible_issues(&self) -> Vec<&IssueSelectPopupIssue> {
        let Some(project) = self.projects.get(self.focused_project_index) else {
            return Vec::new();
        };

        self.issues
            .iter()
            .filter(|issue| issue.project_id == project.id)
            .collect()
    }

    fn focused_issue_id(&self) -> Option<u16> {
        self.focused_issue().map(|issue| issue.issue_id.get())
    }
}

pub enum EventProcessResult {
    Entered { issue_id: u16 },
    Quited,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use crate::app::Action;

    // TODO: 以下はStoreに任意のproject/issueを積むActionが追加されたら結合テストを追加する
    //   - 複数projectでのj/kによるproject列のフォーカス移動とissue列フォーカスのリセット
    //   - project切り替えに追従してプレビュー(widget_state)が更新されること
    //   - updateでStoreから消えたissueにフォーカスしていた場合のフォーカス丸め込み

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
        store.consume_action(Action::LoadProjects);
        store.consume_action(Action::LoadIssue { id: 1 });
        store.consume_action(Action::LoadIssue { id: 2 });
        store.consume_action(Action::LoadIssue { id: 3 });
        store
    }

    #[test]
    fn new_focuses_given_issue_and_its_project() {
        let component = IssueSelectPopupComponent::new(&store(), 2);

        let widget = component.create_widget();

        assert_eq!(widget.focused_project_index, 0);
        assert_eq!(widget.focused_issue_index, 1);
        assert_eq!(widget.focused_column, IssueSelectPopupFocusColumn::Project);
        assert_eq!(widget.projects[0].name, "Sample Project");
        assert_eq!(
            widget
                .issues
                .iter()
                .map(|issue| issue.issue_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn new_falls_back_to_first_issue_when_focused_issue_is_not_in_store() {
        let component = IssueSelectPopupComponent::new(&store(), 404);

        let widget = component.create_widget();

        assert_eq!(widget.focused_project_index, 0);
        assert_eq!(widget.focused_issue_index, 0);
    }

    #[test]
    fn update_keeps_focused_issue_across_store_updates() {
        let mut component = IssueSelectPopupComponent::new(&store(), 3);

        component.update(&store(), AREA);

        let widget = component.create_widget();
        assert_eq!(widget.focused_issue_index, 2);
        assert_eq!(widget.issues[widget.focused_issue_index].issue_id, 3);
    }

    #[test]
    fn update_reflects_issue_subject_updated_in_store() {
        let mut component = IssueSelectPopupComponent::new(&store(), 1);
        let mut store = store();
        store.consume_action(Action::UpdateIssue {
            id: 1,
            body: "updated description".to_string(),
        });

        component.update(&store, AREA);

        let widget = component.create_widget();
        assert_eq!(widget.issues[0].description, "updated description");
    }

    #[test]
    fn create_widget_passes_only_focused_project_issues() {
        let component = IssueSelectPopupComponent {
            projects: vec![
                IssueSelectPopupProject::new(1, "frontend"),
                IssueSelectPopupProject::new(2, "backend"),
            ],
            issues: vec![
                IssueSelectPopupIssue::new(1, 101, "Frontend issue", ""),
                IssueSelectPopupIssue::new(2, 201, "Backend issue 1", ""),
                IssueSelectPopupIssue::new(2, 202, "Backend issue 2", ""),
            ],
            focused_project_index: 1,
            focused_issue_index: 0,
            focused_column: IssueSelectPopupFocusColumn::Issue,
            widget_state: IssueSelectPopupWidgetState::new(),
        };

        let widget = component.create_widget();

        assert_eq!(
            widget
                .issues
                .iter()
                .map(|issue| issue.issue_id.get())
                .collect::<Vec<_>>(),
            vec![201, 202]
        );
    }

    #[test]
    fn process_event_l_and_h_move_between_project_and_issue_columns() {
        let mut component = IssueSelectPopupComponent::new(&store(), 1);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('l')))
                .is_none()
        );
        assert_eq!(
            component.create_widget().focused_column,
            IssueSelectPopupFocusColumn::Issue
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('h')))
                .is_none()
        );
        assert_eq!(
            component.create_widget().focused_column,
            IssueSelectPopupFocusColumn::Project
        );
    }

    #[test]
    fn process_event_j_and_k_move_issue_focus_when_issue_column_is_focused() {
        let mut component = IssueSelectPopupComponent::new(&store(), 1);
        assert!(
            component
                .process_event(key_event(KeyCode::Char('l')))
                .is_none()
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.create_widget().focused_issue_index, 1);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.create_widget().focused_issue_index, 0);
    }

    #[test]
    fn process_event_j_stops_at_last_issue() {
        let mut component = IssueSelectPopupComponent::new(&store(), 3);
        assert!(
            component
                .process_event(key_event(KeyCode::Char('l')))
                .is_none()
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );

        assert_eq!(component.create_widget().focused_issue_index, 2);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let mut component = IssueSelectPopupComponent::new(&store(), 1);

        let result = component.process_event(key_event(KeyCode::Char('q')));

        assert!(matches!(result, Some(EventProcessResult::Quited)));
    }

    #[test]
    fn process_event_enter_returns_focused_issue_id() {
        let mut component = IssueSelectPopupComponent::new(&store(), 1);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('l')))
                .is_none()
        );
        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(
            result,
            Some(EventProcessResult::Entered { issue_id: 2 })
        ));
    }

    #[test]
    fn process_event_returns_none_for_non_key_event() {
        let mut component = IssueSelectPopupComponent::new(&store(), 1);

        let result = component.process_event(Event::Resize(80, 24));

        assert!(result.is_none());
    }

    #[test]
    fn process_event_ignores_unhandled_key() {
        let mut component = IssueSelectPopupComponent::new(&store(), 2);

        let result = component.process_event(key_event(KeyCode::Char('x')));

        assert!(result.is_none());
        assert_eq!(component.create_widget().focused_issue_index, 1);
    }
}
