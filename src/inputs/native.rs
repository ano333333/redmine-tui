use crossterm::event::{
    KeyCode as CrosstermKeyCode, KeyEvent as CrosstermKeyEvent,
    KeyModifiers as CrosstermKeyModifiers,
};

use super::*;

/// crosstermのキー入力をplatform非依存の入力へ変換する。
pub fn convert_key(key: CrosstermKeyEvent) -> Option<InputEvent> {
    match key.code {
        CrosstermKeyCode::Char(c) => {
            let modifiers = key.modifiers;
            if modifiers.contains(CrosstermKeyModifiers::ALT) {
                return None;
            }
            Some(InputEvent::Key(KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers {
                    control: modifiers.contains(CrosstermKeyModifiers::CONTROL),
                    shift: modifiers.contains(CrosstermKeyModifiers::SHIFT),
                },
            )))
        }
        code => {
            if key.modifiers != CrosstermKeyModifiers::NONE {
                return None;
            }
            let code = match code {
                CrosstermKeyCode::Enter => KeyCode::Enter,
                CrosstermKeyCode::Esc => KeyCode::Esc,
                CrosstermKeyCode::Tab => KeyCode::Tab,
                CrosstermKeyCode::BackTab => KeyCode::BackTab,
                _ => return None,
            };
            Some(InputEvent::Key(KeyEvent::new(code, KeyModifiers::none())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode as CKey, KeyEvent as CKeyEvent, KeyModifiers as CMods};

    fn ckey(code: CKey) -> CKeyEvent {
        CKeyEvent::new(code, CMods::NONE)
    }

    fn ckey_mods(code: CKey, modifiers: CMods) -> CKeyEvent {
        CKeyEvent::new(code, modifiers)
    }

    fn event(code: KeyCode, modifiers: KeyModifiers) -> Option<InputEvent> {
        Some(InputEvent::Key(KeyEvent::new(code, modifiers)))
    }

    #[test]
    fn converts_supported_keys() {
        let cases: Vec<(CKeyEvent, Option<InputEvent>)> = vec![
            (
                ckey(CKey::Char('a')),
                event(KeyCode::Char('a'), KeyModifiers::none()),
            ),
            (
                ckey_mods(CKey::Char('s'), CMods::CONTROL),
                event(KeyCode::Char('s'), KeyModifiers::control()),
            ),
            (
                ckey_mods(CKey::Char('D'), CMods::SHIFT),
                event(KeyCode::Char('D'), KeyModifiers::shift()),
            ),
            (
                ckey_mods(CKey::Char('x'), CMods::CONTROL | CMods::SHIFT),
                Some(InputEvent::Key(KeyEvent::new(
                    KeyCode::Char('x'),
                    KeyModifiers {
                        control: true,
                        shift: true,
                    },
                ))),
            ),
            (ckey_mods(CKey::Char('x'), CMods::ALT), None),
            (
                ckey(CKey::Enter),
                event(KeyCode::Enter, KeyModifiers::none()),
            ),
            (ckey(CKey::Esc), event(KeyCode::Esc, KeyModifiers::none())),
            (ckey(CKey::Tab), event(KeyCode::Tab, KeyModifiers::none())),
            (
                ckey(CKey::BackTab),
                event(KeyCode::BackTab, KeyModifiers::none()),
            ),
            (ckey_mods(CKey::Enter, CMods::SHIFT), None),
            (ckey_mods(CKey::Tab, CMods::CONTROL), None),
            (ckey(CKey::Up), None),
            (ckey(CKey::Down), None),
            (ckey(CKey::Left), None),
            (ckey(CKey::Right), None),
            (ckey(CKey::Backspace), None),
            (ckey_mods(CKey::Backspace, CMods::SHIFT), None),
            (ckey(CKey::F(1)), None),
        ];
        for (input, expected) in cases {
            assert_eq!(convert_key(input), expected, "input: {:?}", input);
        }
    }
}
