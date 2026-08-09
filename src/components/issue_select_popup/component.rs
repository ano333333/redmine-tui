use crossterm::event::{Event, KeyCode};

use super::widget::{
    IssueSelectPopupFocusColumn, IssueSelectPopupIssue, IssueSelectPopupProject,
    IssueSelectPopupWidget,
};

pub struct IssueSelectPopupComponent {
    projects: Vec<IssueSelectPopupProject>,
    issues: Vec<IssueSelectPopupIssue>,
    focused_project_index: usize,
    focused_issue_index: usize,
    focused_column: IssueSelectPopupFocusColumn,
}

impl IssueSelectPopupComponent {
    pub fn new(
        projects: &[(u16, String)],
        issues: &[IssueSelectPopupIssue],
        focused_project_index: usize,
        focused_issue_index: usize,
    ) -> Self {
        Self {
            projects: projects
                .iter()
                .map(|(id, name)| IssueSelectPopupProject::new(*id, name.clone()))
                .collect(),
            issues: issues.to_vec(),
            focused_project_index,
            focused_issue_index,
            focused_column: IssueSelectPopupFocusColumn::Project,
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
            _ => {}
        }

        None
    }

    pub fn create_widget<'a>(&'a self) -> IssueSelectPopupWidget<'a> {
        IssueSelectPopupWidget::new(
            &self.projects,
            &self.issues,
            self.focused_project_index,
            self.focused_issue_index,
            self.focused_column,
        )
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
        let Some(project) = self.projects.get(self.focused_project_index) else {
            return 0;
        };

        self.issues
            .iter()
            .filter(|issue| issue.project_id == project.id)
            .count()
    }
}

pub enum EventProcessResult {
    Quited,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn projects() -> Vec<(u16, String)> {
        vec![
            (1, "redmine-tui".to_string()),
            (2, "backend".to_string()),
            (3, "docs".to_string()),
        ]
    }

    fn issues() -> Vec<IssueSelectPopupIssue> {
        vec![
            IssueSelectPopupIssue::new(1, 101, "Issue selector popup", "description"),
            IssueSelectPopupIssue::new(2, 204, "API shape", "description"),
            IssueSelectPopupIssue::new(2, 205, "Keyboard flow", "description"),
        ]
    }

    #[test]
    fn create_widget_splits_projects_and_issues_with_focus_state() {
        let component = IssueSelectPopupComponent::new(&projects(), &issues(), 1, 0);

        let widget = component.create_widget();

        assert_eq!(widget.focused_project_index, 1);
        assert_eq!(widget.focused_issue_index, 0);
        assert_eq!(widget.focused_column, IssueSelectPopupFocusColumn::Project);
        assert_eq!(widget.projects[1].name, "backend");
        assert_eq!(widget.issues[1].issue_id, 204);
    }

    #[test]
    fn process_event_l_and_h_move_between_project_and_issue_columns() {
        let mut component = IssueSelectPopupComponent::new(&projects(), &issues(), 1, 0);

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
    fn process_event_j_and_k_move_project_focus_and_reset_issue_focus() {
        let mut component = IssueSelectPopupComponent::new(&projects(), &issues(), 0, 1);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );

        let widget = component.create_widget();
        assert_eq!(widget.focused_project_index, 1);
        assert_eq!(widget.focused_issue_index, 0);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );

        let widget = component.create_widget();
        assert_eq!(widget.focused_project_index, 0);
        assert_eq!(widget.focused_issue_index, 0);
    }

    #[test]
    fn process_event_j_and_k_move_issue_focus_when_issue_column_is_focused() {
        let mut component = IssueSelectPopupComponent::new(&projects(), &issues(), 1, 0);
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
    fn process_event_q_returns_quited() {
        let mut component = IssueSelectPopupComponent::new(&projects(), &issues(), 1, 0);

        let result = component.process_event(key_event(KeyCode::Char('q')));

        assert!(matches!(result, Some(EventProcessResult::Quited)));
    }
}
