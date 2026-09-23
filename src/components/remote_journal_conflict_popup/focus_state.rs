use super::widget::RemoteJournalConflictButton;
use crate::platform::input::{InputEvent, KeyCode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Remote Journal競合popup内でフォーカスされている対象。
pub enum FocusTarget {
    Choice(usize),
    Button(RemoteJournalConflictButton),
}

/// フォーカス状態がキー入力から生成する操作結果。
pub enum EventProcessResult {
    Selected(usize),
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

/// Remote Journal競合popupの選択肢とボタンのフォーカスを管理する。
pub struct FocusState {
    choice_count: usize,
    target: FocusTarget,
}

impl FocusState {
    /// 選択肢数と初期選択肢のindexからフォーカス状態を作成する。
    pub fn new(choice_count: usize, initial_choice: usize) -> Self {
        Self {
            choice_count,
            target: FocusTarget::Choice(initial_choice),
        }
    }

    /// キーイベントをフォーカス移動または確定・キャンセル操作へ変換する。
    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        let action = self.action_from_event(event)?;
        self.apply_action(action)
    }

    pub fn target(&self) -> FocusTarget {
        self.target
    }

    pub fn focused_button(&self) -> Option<RemoteJournalConflictButton> {
        match self.target {
            FocusTarget::Choice(_) => None,
            FocusTarget::Button(button) => Some(button),
        }
    }

    fn action_from_event(&self, event: InputEvent) -> Option<Action> {
        let InputEvent::Key(key) = event;
        match key.code {
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Esc => Some(Action::Quit),
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
        match self.target {
            FocusTarget::Choice(_) => {
                self.target = FocusTarget::Button(RemoteJournalConflictButton::Continue);
            }
            FocusTarget::Button(_) => {}
        }
    }

    fn move_up(&mut self) {
        match self.target {
            FocusTarget::Choice(index) => {
                if index > 0 {
                    self.target = FocusTarget::Choice(index - 1);
                }
            }
            FocusTarget::Button(_) => {
                if self.choice_count > 0 {
                    self.target = FocusTarget::Choice(0);
                }
            }
        }
    }

    fn move_left(&mut self) {
        self.target = match self.target {
            FocusTarget::Choice(index) => FocusTarget::Choice(index.saturating_sub(1)),
            FocusTarget::Button(RemoteJournalConflictButton::Continue) => {
                FocusTarget::Choice(self.choice_count.saturating_sub(1))
            }
            FocusTarget::Button(RemoteJournalConflictButton::Cancel) => {
                FocusTarget::Button(RemoteJournalConflictButton::Continue)
            }
        };
    }

    fn move_right(&mut self) {
        self.target = match self.target {
            FocusTarget::Choice(index) if index + 1 < self.choice_count => {
                FocusTarget::Choice(index + 1)
            }
            FocusTarget::Choice(_) => FocusTarget::Button(RemoteJournalConflictButton::Continue),
            FocusTarget::Button(_) => FocusTarget::Button(RemoteJournalConflictButton::Cancel),
        };
    }

    fn enter(&self) -> Option<EventProcessResult> {
        match self.target {
            FocusTarget::Choice(index) => Some(EventProcessResult::Selected(index)),
            FocusTarget::Button(RemoteJournalConflictButton::Cancel) => {
                Some(EventProcessResult::Canceled)
            }
            FocusTarget::Button(RemoteJournalConflictButton::Continue) => {
                Some(EventProcessResult::Continued)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    #[test]
    fn new_focuses_local_choice_and_no_button() {
        let state = FocusState::new(2, 0);

        assert_eq!(state.target(), FocusTarget::Choice(0));
        assert_eq!(state.focused_button(), None);
    }

    #[test]
    fn h_and_l_move_target_between_choices() {
        let mut state = FocusState::new(2, 0);

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(state.target(), FocusTarget::Choice(1));
        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(state.target(), FocusTarget::Choice(0));
    }

    #[test]
    fn l_from_remote_focuses_continue_and_h_returns_to_remote() {
        let mut state = FocusState::new(2, 0);
        state.process_event(key_event(KeyCode::Char('l')));

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Button(RemoteJournalConflictButton::Continue)
        );

        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(state.target(), FocusTarget::Choice(1));
    }

    #[test]
    fn j_from_choice_focuses_continue_button() {
        let mut state = FocusState::new(2, 0);
        assert!(state.process_event(key_event(KeyCode::Char('j'))).is_none());

        assert_eq!(
            state.target(),
            FocusTarget::Button(RemoteJournalConflictButton::Continue)
        );
        assert_eq!(
            state.focused_button(),
            Some(RemoteJournalConflictButton::Continue)
        );
    }

    #[test]
    fn k_from_button_returns_to_last_choice() {
        let mut state = FocusState::new(2, 0);
        state.process_event(key_event(KeyCode::Char('j')));

        assert!(state.process_event(key_event(KeyCode::Char('k'))).is_none());

        assert_eq!(state.target(), FocusTarget::Choice(0));
        assert_eq!(state.focused_button(), None);
    }

    #[test]
    fn h_and_l_move_target_between_cancel_and_continue_buttons() {
        let mut state = FocusState::new(2, 0);
        state.process_event(key_event(KeyCode::Char('j')));

        assert!(state.process_event(key_event(KeyCode::Char('l'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Button(RemoteJournalConflictButton::Cancel)
        );
        assert_eq!(
            state.focused_button(),
            Some(RemoteJournalConflictButton::Cancel)
        );

        assert!(state.process_event(key_event(KeyCode::Char('h'))).is_none());
        assert_eq!(
            state.target(),
            FocusTarget::Button(RemoteJournalConflictButton::Continue)
        );
    }

    #[test]
    fn enter_on_choice_returns_selected() {
        let mut state = FocusState::new(2, 0);
        state.process_event(key_event(KeyCode::Char('l')));

        assert!(matches!(
            state.process_event(key_event(KeyCode::Enter)),
            Some(EventProcessResult::Selected(1))
        ));
    }

    #[test]
    fn enter_on_cancel_returns_canceled() {
        let mut state = FocusState::new(2, 0);
        state.process_event(key_event(KeyCode::Char('j')));
        state.process_event(key_event(KeyCode::Char('l')));

        assert!(matches!(
            state.process_event(key_event(KeyCode::Enter)),
            Some(EventProcessResult::Canceled)
        ));
    }

    #[test]
    fn enter_on_continue_returns_continued() {
        let mut state = FocusState::new(2, 0);
        state.process_event(key_event(KeyCode::Char('j')));

        assert!(matches!(
            state.process_event(key_event(KeyCode::Enter)),
            Some(EventProcessResult::Continued)
        ));
    }

    #[test]
    fn q_and_esc_return_canceled() {
        let mut state = FocusState::new(2, 0);

        assert!(matches!(
            state.process_event(key_event(KeyCode::Char('q'))),
            Some(EventProcessResult::Canceled)
        ));
        assert!(matches!(
            state.process_event(key_event(KeyCode::Esc)),
            Some(EventProcessResult::Canceled)
        ));
    }
}
