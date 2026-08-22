use crossterm::event::{Event, KeyCode};

use super::widget::{IssuePropertyConflictButton, IssuePropertyConflictFocus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusTarget {
    Cell {
        row_index: usize,
        column: IssuePropertyConflictFocus,
    },
    Button(IssuePropertyConflictButton),
}

pub enum EventProcessResult {
    Selected {
        row_index: usize,
        choice: IssuePropertyConflictFocus,
    },
    Canceled,
    Continued,
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
    row_count: usize,
    target: FocusTarget,
    last_cell_column: IssuePropertyConflictFocus,
}

impl FocusState {
    /// フォーカス可能な行数から初期フォーカス状態を作成する。
    pub fn new(row_count: usize) -> Self {
        Self {
            row_count,
            target: FocusTarget::Cell {
                row_index: 0,
                column: IssuePropertyConflictFocus::After,
            },
            last_cell_column: IssuePropertyConflictFocus::After,
        }
    }

    /// キーイベントをフォーカス移動または確定イベントへ変換する。
    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    /// 現在フォーカスされている対象を返す。
    pub fn target(&self) -> FocusTarget {
        self.target
    }

    /// 現在フォーカスされているボタンを返す。
    pub fn focused_button(&self) -> Option<IssuePropertyConflictButton> {
        match self.target {
            FocusTarget::Cell { .. } => None,
            FocusTarget::Button(button) => Some(button),
        }
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
            Action::MoveDown => self.move_down(),
            Action::MoveUp => self.move_up(),
            Action::MoveLeft => self.move_left(),
            Action::MoveRight => self.move_right(),
            Action::Enter => return self.enter(),
            Action::Quit => return Some(EventProcessResult::Canceled),
        }
        None
    }

    fn move_down(&mut self) {
        if self.row_count == 0 {
            self.target = FocusTarget::Button(IssuePropertyConflictButton::Continue);
            return;
        }

        if let FocusTarget::Cell { row_index, column } = self.target {
            self.last_cell_column = column;
            if row_index + 1 < self.row_count {
                self.target = FocusTarget::Cell {
                    row_index: row_index + 1,
                    column,
                };
            } else {
                self.target = FocusTarget::Button(IssuePropertyConflictButton::Continue);
            }
        }
    }

    fn move_up(&mut self) {
        match self.target {
            FocusTarget::Cell { row_index, column } => {
                self.last_cell_column = column;
                if row_index > 0 {
                    self.target = FocusTarget::Cell {
                        row_index: row_index - 1,
                        column,
                    };
                }
            }
            FocusTarget::Button(_) => {
                if self.row_count > 0 {
                    self.target = FocusTarget::Cell {
                        row_index: self.row_count - 1,
                        column: self.last_cell_column,
                    };
                }
            }
        }
    }

    fn move_left(&mut self) {
        match self.target {
            FocusTarget::Cell { row_index, .. } => {
                self.last_cell_column = IssuePropertyConflictFocus::After;
                self.target = FocusTarget::Cell {
                    row_index,
                    column: IssuePropertyConflictFocus::After,
                };
            }
            FocusTarget::Button(_) => {
                self.target = FocusTarget::Button(IssuePropertyConflictButton::Cancel);
            }
        }
    }

    fn move_right(&mut self) {
        match self.target {
            FocusTarget::Cell { row_index, .. } => {
                self.last_cell_column = IssuePropertyConflictFocus::Server;
                self.target = FocusTarget::Cell {
                    row_index,
                    column: IssuePropertyConflictFocus::Server,
                };
            }
            FocusTarget::Button(_) => {
                self.target = FocusTarget::Button(IssuePropertyConflictButton::Continue);
            }
        }
    }

    fn enter(&self) -> Option<EventProcessResult> {
        match self.target {
            FocusTarget::Cell { row_index, column } => Some(EventProcessResult::Selected {
                row_index,
                choice: column,
            }),
            FocusTarget::Button(IssuePropertyConflictButton::Cancel) => {
                Some(EventProcessResult::Canceled)
            }
            FocusTarget::Button(IssuePropertyConflictButton::Continue) => {
                Some(EventProcessResult::Continued)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn new_focuses_first_after_cell_and_no_button() {
        let state = FocusState::new(2);

        assert_eq!(
            state.target(),
            FocusTarget::Cell {
                row_index: 0,
                column: IssuePropertyConflictFocus::After,
            }
        );
        assert_eq!(state.focused_button(), None);
    }

    #[test]
    fn h_and_l_move_target_between_after_and_server_columns() {
        let mut state = FocusState::new(2);

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Cell {
                row_index: 0,
                column: IssuePropertyConflictFocus::Server,
            }
        );
        assert_eq!(state.focused_button(), None);

        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Cell {
                row_index: 0,
                column: IssuePropertyConflictFocus::After,
            }
        );
    }

    #[test]
    fn j_and_k_move_target_between_rows_in_current_column() {
        let mut state = FocusState::new(3);
        state.process_event(key_event(KeyCode::Char('l')));

        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Cell {
                row_index: 1,
                column: IssuePropertyConflictFocus::Server,
            }
        );

        assert!(state.process_event(key_event(KeyCode::Char('k'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Cell {
                row_index: 0,
                column: IssuePropertyConflictFocus::Server,
            }
        );
    }

    #[test]
    fn j_from_last_row_focuses_continue_button() {
        let mut state = FocusState::new(2);
        state.process_event(key_event(KeyCode::Char('j')));

        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());

        assert_eq!(
            state.target(),
            FocusTarget::Button(IssuePropertyConflictButton::Continue)
        );
        assert_eq!(
            state.focused_button(),
            Some(IssuePropertyConflictButton::Continue)
        );
    }

    #[test]
    fn h_and_l_move_target_between_cancel_and_continue_buttons() {
        let mut state = FocusState::new(1);
        state.process_event(key_event(KeyCode::Char('j')));

        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Button(IssuePropertyConflictButton::Cancel)
        );
        assert_eq!(
            state.focused_button(),
            Some(IssuePropertyConflictButton::Cancel)
        );

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Button(IssuePropertyConflictButton::Continue)
        );
        assert_eq!(
            state.focused_button(),
            Some(IssuePropertyConflictButton::Continue)
        );
    }

    #[test]
    fn k_from_button_returns_to_last_row_in_last_cell_column() {
        let mut state = FocusState::new(2);
        state.process_event(key_event(KeyCode::Char('l')));
        state.process_event(key_event(KeyCode::Char('j')));
        state.process_event(key_event(KeyCode::Char('j')));

        assert!(state.process_event(key_event(KeyCode::Char('k'))).is_none());

        assert_eq!(
            state.target(),
            FocusTarget::Cell {
                row_index: 1,
                column: IssuePropertyConflictFocus::Server,
            }
        );
        assert_eq!(state.focused_button(), None);
    }

    #[test]
    fn zero_rows_focuses_continue_button_on_j() {
        let mut state = FocusState::new(0);

        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());

        assert_eq!(
            state.target(),
            FocusTarget::Button(IssuePropertyConflictButton::Continue)
        );
        assert_eq!(
            state.focused_button(),
            Some(IssuePropertyConflictButton::Continue)
        );
    }
}
