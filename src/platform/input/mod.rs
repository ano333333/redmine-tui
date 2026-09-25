#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
}

/// 現在の操作で区別が必要な修飾状態だけを表す。
///
/// 複合修飾キーを追加する場合は、入力adapterとComponentの双方で扱いを定義してから表現を拡張する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    control: bool,
    shift: bool,
}

impl KeyModifiers {
    pub const fn none() -> Self {
        Self {
            control: false,
            shift: false,
        }
    }
    pub const fn control() -> Self {
        Self {
            control: true,
            shift: false,
        }
    }
    pub const fn shift() -> Self {
        Self {
            control: false,
            shift: true,
        }
    }
    pub fn is_control(&self) -> bool {
        self.control
    }
    pub fn is_shift(&self) -> bool {
        self.shift
    }
}

#[cfg(feature = "native")]
pub mod native;

#[cfg(feature = "web-demo")]
pub mod web;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_key_is_comparable() {
        let a = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::none());
        let b = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::none());
        assert_eq!(a, b);
        assert_ne!(a, KeyEvent::new(KeyCode::Char('k'), KeyModifiers::none()));
    }

    #[test]
    fn char_key_clone_preserves_identity() {
        let a = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::none());
        let cloned = a.clone();
        assert_eq!(a, cloned);
    }

    #[test]
    fn non_char_keys_are_comparable() {
        assert_eq!(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::none()),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::none())
        );
        assert_ne!(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::none()),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::none())
        );
        assert_eq!(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::none()),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::none())
        );
        assert_ne!(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::none()),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::none())
        );
    }

    #[test]
    fn modified_keys_differ_from_unmodified() {
        let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::control());
        assert_eq!(
            ctrl_s,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::control())
        );
        assert_ne!(
            ctrl_s,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::none())
        );
        assert_ne!(
            ctrl_s,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::control())
        );
    }

    #[test]
    fn shifted_char_key_is_comparable() {
        let shift_d = KeyEvent::new(KeyCode::Char('D'), KeyModifiers::shift());
        assert_eq!(
            shift_d,
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::shift())
        );
        assert_ne!(
            shift_d,
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::none())
        );
        assert!(shift_d.modifiers.is_shift());
        assert!(!shift_d.modifiers.is_control());
        let cloned = shift_d.clone();
        assert_eq!(shift_d, cloned);
    }
}
