use crossterm::event::{Event, KeyCode};

use super::widget::IssueSelectPopupFocusColumn;

pub enum EventProcessResult {
    Selected,
    Quited,
    ProjectChanged,
    PreviousPageRequested,
    NextPageRequested,
    RetryRequested,
}

enum Action {
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    Enter,
    Quit,
    Retry,
}

pub struct FocusState {
    project_issue_counts: Vec<usize>,
    focused_project_index: usize,
    focused_issue_index: usize,
    focused_column: IssueSelectPopupFocusColumn,
    empty_issue_column_enterable: bool,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            project_issue_counts: Vec::new(),
            focused_project_index: 0,
            focused_issue_index: 0,
            focused_column: IssueSelectPopupFocusColumn::Project,
            empty_issue_column_enterable: false,
        }
    }

    pub fn replace_project_issue_counts(&mut self, project_issue_counts: Vec<usize>) {
        self.project_issue_counts = project_issue_counts;
        self.clamp_focus();
    }

    pub fn focus_issue(&mut self, project_index: usize, issue_index: usize) {
        self.focused_project_index = project_index;
        self.focused_issue_index = issue_index;
        self.clamp_focus();
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    pub fn focused_project_index(&self) -> usize {
        self.focused_project_index
    }

    pub fn focused_issue_index(&self) -> usize {
        self.focused_issue_index
    }

    pub fn focused_column(&self) -> IssueSelectPopupFocusColumn {
        self.focused_column
    }

    pub fn reset_issue_focus(&mut self) {
        self.focused_issue_index = 0;
    }

    pub fn focus_project_column(&mut self) {
        self.focused_column = IssueSelectPopupFocusColumn::Project;
    }

    pub(super) fn set_empty_issue_column_enterable(&mut self, enterable: bool) {
        self.empty_issue_column_enterable = enterable;
    }

    fn action_from_event(&self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('r') => Some(Action::Retry),
            KeyCode::Enter => Some(Action::Enter),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::MoveDown => match self.focused_column {
                IssueSelectPopupFocusColumn::Project => {
                    if self.focus_next_project() {
                        return Some(EventProcessResult::ProjectChanged);
                    }
                }
                IssueSelectPopupFocusColumn::Issue => {
                    if !self.focus_next_issue() {
                        return Some(EventProcessResult::NextPageRequested);
                    }
                }
            },
            Action::MoveUp => match self.focused_column {
                IssueSelectPopupFocusColumn::Project => {
                    if self.focus_previous_project() {
                        return Some(EventProcessResult::ProjectChanged);
                    }
                }
                IssueSelectPopupFocusColumn::Issue => {
                    if !self.focus_previous_issue() {
                        return Some(EventProcessResult::PreviousPageRequested);
                    }
                }
            },
            Action::MoveLeft => {
                self.focused_column = IssueSelectPopupFocusColumn::Project;
            }
            Action::MoveRight => {
                if self.focused_project_issue_count() > 0 || self.empty_issue_column_enterable {
                    self.focused_column = IssueSelectPopupFocusColumn::Issue;
                }
            }
            Action::Quit => {
                return Some(EventProcessResult::Quited);
            }
            Action::Enter => {
                if self.focused_project_issue_count() > 0 {
                    return Some(EventProcessResult::Selected);
                }
            }
            Action::Retry => return Some(EventProcessResult::RetryRequested),
        }

        None
    }

    fn clamp_focus(&mut self) {
        self.focused_project_index = self
            .focused_project_index
            .min(self.project_issue_counts.len().saturating_sub(1));
        self.focused_issue_index = self
            .focused_issue_index
            .min(self.focused_project_issue_count().saturating_sub(1));
    }

    fn focus_next_project(&mut self) -> bool {
        if self.focused_project_index + 1 < self.project_issue_counts.len() {
            self.focused_project_index += 1;
            self.focused_issue_index = 0;
            true
        } else {
            false
        }
    }

    fn focus_previous_project(&mut self) -> bool {
        if self.focused_project_index > 0 {
            self.focused_project_index -= 1;
            self.focused_issue_index = 0;
            true
        } else {
            false
        }
    }

    fn focus_next_issue(&mut self) -> bool {
        if self.focused_issue_index + 1 < self.focused_project_issue_count() {
            self.focused_issue_index += 1;
            true
        } else {
            false
        }
    }

    fn focus_previous_issue(&mut self) -> bool {
        if self.focused_issue_index > 0 {
            self.focused_issue_index -= 1;
            true
        } else {
            false
        }
    }

    fn focused_project_issue_count(&self) -> usize {
        self.project_issue_counts
            .get(self.focused_project_index)
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn state_focused_on(project_index: usize, issue_index: usize) -> FocusState {
        let mut state = FocusState::new();
        state.replace_project_issue_counts(vec![1, 2]);
        state.focus_issue(project_index, issue_index);
        state
    }

    #[test]
    fn focus_issue_sets_project_and_issue_indices() {
        let state = state_focused_on(1, 1);

        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 1);
    }

    #[test]
    fn focus_issue_clamps_indices_to_counts() {
        let state = state_focused_on(4, 4);

        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 1);
    }

    #[test]
    fn replace_project_issue_counts_clamps_current_focus() {
        let mut state = state_focused_on(1, 1);

        state.replace_project_issue_counts(vec![1]);

        assert_eq!(state.focused_project_index(), 0);
        assert_eq!(state.focused_issue_index(), 0);
    }

    #[test]
    fn process_event_l_and_h_move_between_project_and_issue_columns() {
        let mut state = state_focused_on(0, 0);

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(state.focused_column(), IssueSelectPopupFocusColumn::Issue);

        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(state.focused_column(), IssueSelectPopupFocusColumn::Project);
    }

    #[test]
    fn focus_project_column_keeps_indices_and_moves_only_the_column_focus() {
        let mut state = state_focused_on(1, 1);
        state.process_event(key_event(KeyCode::Char('l')));

        state.focus_project_column();

        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 1);
        assert_eq!(state.focused_column(), IssueSelectPopupFocusColumn::Project);
    }

    #[test]
    fn process_event_l_keeps_project_column_focused_when_project_has_no_issues() {
        let mut state = FocusState::new();
        state.replace_project_issue_counts(vec![0]);

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());

        assert_eq!(state.focused_column(), IssueSelectPopupFocusColumn::Project);
    }

    #[test]
    fn process_event_l_enters_an_empty_issue_column_when_component_allows_it() {
        let mut state = FocusState::new();
        state.replace_project_issue_counts(vec![0]);
        state.set_empty_issue_column_enterable(true);

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(state.focused_column(), IssueSelectPopupFocusColumn::Issue);
        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::PreviousPageRequested)
        ));
    }

    #[test]
    fn process_event_j_and_k_move_project_focus_and_reset_issue_focus() {
        let mut state = state_focused_on(0, 0);

        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::ProjectChanged)
        ));
        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 0);

        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::ProjectChanged)
        ));
        assert_eq!(state.focused_project_index(), 0);
        assert_eq!(state.focused_issue_index(), 0);
    }

    #[test]
    fn process_event_j_and_k_move_issue_focus_when_issue_column_is_focused() {
        let mut state = state_focused_on(1, 0);
        state.process_event(key_event(KeyCode::Char('l')));

        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());
        assert_eq!(state.focused_issue_index(), 1);

        assert!(state.process_event(key_event(KeyCode::Char('k'))).is_none());
        assert_eq!(state.focused_issue_index(), 0);
    }

    #[test]
    fn process_event_j_stops_at_last_issue() {
        let mut state = state_focused_on(1, 1);
        state.process_event(key_event(KeyCode::Char('l')));

        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::NextPageRequested)
        ));

        assert_eq!(state.focused_issue_index(), 1);
    }

    #[test]
    fn process_event_k_at_first_issue_requests_previous_page() {
        let mut state = state_focused_on(1, 0);
        state.process_event(key_event(KeyCode::Char('l')));

        let result = state.process_event(key_event(KeyCode::Char('k')));

        assert!(matches!(
            result,
            Some(EventProcessResult::PreviousPageRequested)
        ));
    }

    #[test]
    fn empty_page_keeps_issue_column_and_reports_both_boundaries() {
        let mut state = state_focused_on(0, 0);
        state.process_event(key_event(KeyCode::Char('l')));
        state.replace_project_issue_counts(vec![0]);

        assert_eq!(state.focused_column(), IssueSelectPopupFocusColumn::Issue);
        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::PreviousPageRequested)
        ));
        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::NextPageRequested)
        ));
    }

    #[test]
    fn process_event_r_requests_retry_without_changing_focus() {
        let mut state = state_focused_on(1, 1);

        let result = state.process_event(key_event(KeyCode::Char('r')));

        assert!(matches!(result, Some(EventProcessResult::RetryRequested)));
        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 1);
    }

    #[test]
    fn moving_project_reports_project_change() {
        let mut state = state_focused_on(0, 0);

        let result = state.process_event(key_event(KeyCode::Char('j')));

        assert!(matches!(result, Some(EventProcessResult::ProjectChanged)));
        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 0);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let mut state = state_focused_on(0, 0);

        let result = state.process_event(key_event(KeyCode::Char('q')));

        assert!(matches!(result, Some(EventProcessResult::Quited)));
    }

    #[test]
    fn process_event_enter_returns_selected_when_issue_is_focused() {
        let mut state = state_focused_on(1, 0);

        let result = state.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Selected)));
    }

    #[test]
    fn process_event_enter_returns_none_when_no_issue_is_focused() {
        let mut state = FocusState::new();
        state.replace_project_issue_counts(vec![0]);

        let result = state.process_event(key_event(KeyCode::Enter));

        assert!(result.is_none());
    }

    #[test]
    fn process_event_returns_none_for_non_key_event() {
        let mut state = state_focused_on(0, 0);

        let result = state.process_event(Event::Resize(80, 24));

        assert!(result.is_none());
    }

    #[test]
    fn process_event_ignores_unhandled_key() {
        let mut state = state_focused_on(1, 1);

        let result = state.process_event(key_event(KeyCode::Char('x')));

        assert!(result.is_none());
        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 1);
    }
}
