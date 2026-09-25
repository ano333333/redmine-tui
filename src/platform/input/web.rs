use ratzilla::event::{KeyCode as RatzillaKeyCode, KeyEvent as RatzillaKeyEvent};

use super::*;

pub fn convert_key(key: RatzillaKeyEvent) -> Option<InputEvent> {
    match key.code {
        RatzillaKeyCode::Char(c) => {
            if key.alt {
                return None;
            }
            Some(InputEvent::Key(KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers {
                    control: key.ctrl,
                    shift: key.shift,
                },
            )))
        }
        RatzillaKeyCode::Enter if !key.ctrl && !key.alt && !key.shift => Some(InputEvent::Key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::none()),
        )),
        RatzillaKeyCode::Esc if !key.ctrl && !key.alt && !key.shift => Some(InputEvent::Key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::none()),
        )),
        RatzillaKeyCode::Tab if !key.ctrl && !key.alt && !key.shift => Some(InputEvent::Key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::none()),
        )),
        // RatzillaにはBackTabがなく、Shift+Tabはshift付きのTabとして届く。
        RatzillaKeyCode::Tab if !key.ctrl && !key.alt && key.shift => Some(InputEvent::Key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::none()),
        )),
        code if !key.ctrl && !key.alt && !key.shift => {
            let code = match code {
                RatzillaKeyCode::Backspace => KeyCode::Backspace,
                RatzillaKeyCode::Delete => KeyCode::Delete,
                RatzillaKeyCode::Left => KeyCode::Left,
                RatzillaKeyCode::Right => KeyCode::Right,
                RatzillaKeyCode::Home => KeyCode::Home,
                RatzillaKeyCode::End => KeyCode::End,
                _ => return None,
            };
            Some(InputEvent::Key(KeyEvent::new(code, KeyModifiers::none())))
        }
        _ => None,
    }
}
