use crossterm::event::{Event, KeyCode};

use super::widget::IssueSelectPopupFocusColumn;

pub enum EventProcessResult {
    Entered,
    Quited,
}

enum Action {
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    Enter,
    Quit,
}

pub struct FocusState {
    project_issue_counts: Vec<usize>,
    focused_project_index: usize,
    focused_issue_index: usize,
    focused_column: IssueSelectPopupFocusColumn,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            project_issue_counts: Vec::new(),
            focused_project_index: 0,
            focused_issue_index: 0,
            focused_column: IssueSelectPopupFocusColumn::Project,
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
            KeyCode::Enter => Some(Action::Enter),
            _ => None,
        }
    }

    fn apply_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::MoveDown => match self.focused_column {
                IssueSelectPopupFocusColumn::Project => self.focus_next_project(),
                IssueSelectPopupFocusColumn::Issue => self.focus_next_issue(),
            },
            Action::MoveUp => match self.focused_column {
                IssueSelectPopupFocusColumn::Project => self.focus_previous_project(),
                IssueSelectPopupFocusColumn::Issue => self.focus_previous_issue(),
            },
            Action::MoveLeft => {
                self.focused_column = IssueSelectPopupFocusColumn::Project;
            }
            Action::MoveRight => {
                self.focused_column = IssueSelectPopupFocusColumn::Issue;
            }
            Action::Quit => {
                return Some(EventProcessResult::Quited);
            }
            Action::Enter => {
                if self.focused_project_issue_count() > 0 {
                    return Some(EventProcessResult::Entered);
                }
            }
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

    fn focus_next_project(&mut self) {
        if self.focused_project_index + 1 < self.project_issue_counts.len() {
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
    fn process_event_j_and_k_move_project_focus_and_reset_issue_focus() {
        let mut state = state_focused_on(0, 0);

        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());
        assert_eq!(state.focused_project_index(), 1);
        assert_eq!(state.focused_issue_index(), 0);

        assert!(state.process_event(key_event(KeyCode::Char('k'))).is_none());
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

        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());

        assert_eq!(state.focused_issue_index(), 1);
    }

    #[test]
    fn process_event_q_returns_quited() {
        let mut state = state_focused_on(0, 0);

        let result = state.process_event(key_event(KeyCode::Char('q')));

        assert!(matches!(result, Some(EventProcessResult::Quited)));
    }

    #[test]
    fn process_event_enter_returns_entered_when_issue_is_focused() {
        let mut state = state_focused_on(1, 0);

        let result = state.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
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
