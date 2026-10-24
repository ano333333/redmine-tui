use std::path::PathBuf;

use ratatui::style::{Modifier, Style};
use ratatui_textarea::{Input, Key, TextArea};

use super::widget::AttachmentInputPopupWidget;
use crate::platform::input::{InputEvent, KeyCode, KeyEvent};

pub enum EventProcessResult {
    Entered { filename: String, path: PathBuf },
    Canceled,
    Handled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Filename,
    Path,
}

/// 添付ファイルとして送るファイルの名前と、読み込むローカルのパスを入力するpopup。
///
/// どちらかが空のままでは確定しない。
pub struct AttachmentInputPopupComponent<'a> {
    filename: TextArea<'a>,
    path: TextArea<'a>,
    focused: Field,
    is_incomplete: bool,
}

impl AttachmentInputPopupComponent<'_> {
    pub fn new() -> Self {
        Self {
            filename: Self::textarea(),
            path: Self::textarea(),
            focused: Field::Filename,
            is_incomplete: false,
        }
    }

    fn textarea() -> TextArea<'static> {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        let InputEvent::Key(key) = event;
        match key.code {
            KeyCode::Enter => Some(self.confirm()),
            KeyCode::Esc => Some(EventProcessResult::Canceled),
            KeyCode::Tab | KeyCode::BackTab => {
                self.focused = match self.focused {
                    Field::Filename => Field::Path,
                    Field::Path => Field::Filename,
                };
                Some(EventProcessResult::Handled)
            }
            KeyCode::Char(_)
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End => {
                let textarea = match self.focused {
                    Field::Filename => &mut self.filename,
                    Field::Path => &mut self.path,
                };
                textarea.input(textarea_input(key));
                self.is_incomplete = false;
                Some(EventProcessResult::Handled)
            }
        }
    }

    pub fn create_widget<'b>(&'b self) -> AttachmentInputPopupWidget<'b> {
        AttachmentInputPopupWidget::new(
            &self.filename,
            &self.path,
            self.focused,
            self.is_incomplete,
        )
    }

    fn confirm(&mut self) -> EventProcessResult {
        let filename = self.filename.lines()[0].clone();
        let path = self.path.lines()[0].clone();
        if filename.is_empty() || path.is_empty() {
            self.is_incomplete = true;
            return EventProcessResult::Handled;
        }
        EventProcessResult::Entered {
            filename,
            path: PathBuf::from(path),
        }
    }
}

// Componentの入力をplatform非依存に保ち、Textareaへ渡すこの境界でだけ専用型へ変換する。
fn textarea_input(key: KeyEvent) -> Input {
    let textarea_key = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Enter | KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
            unreachable!("popup handles {:?} before text input", key.code)
        }
    };
    Input {
        key: textarea_key,
        ctrl: key.modifiers.is_control(),
        alt: false,
        shift: key.modifiers.is_shift(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::KeyModifiers;
    use crate::test_support::render_snapshot;

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn type_text(component: &mut AttachmentInputPopupComponent<'_>, text: &str) {
        for c in text.chars() {
            component.process_event(key_event(KeyCode::Char(c)));
        }
    }

    #[test]
    fn enter_returns_the_filename_and_the_path() {
        let mut component = AttachmentInputPopupComponent::new();
        type_text(&mut component, "screen.png");
        component.process_event(key_event(KeyCode::Tab));
        type_text(&mut component, "/tmp/q.png");

        let result = component.process_event(key_event(KeyCode::Enter));

        let Some(EventProcessResult::Entered { filename, path }) = result else {
            panic!("expected Entered");
        };
        assert_eq!(filename, "screen.png");
        assert_eq!(path, PathBuf::from("/tmp/q.png"));
    }

    #[test]
    fn enter_with_an_empty_field_keeps_the_popup_open_until_edited() {
        let mut component = AttachmentInputPopupComponent::new();
        type_text(&mut component, "screen.png");

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert!(component.is_incomplete);
        component.process_event(key_event(KeyCode::Tab));
        type_text(&mut component, "/");
        assert!(!component.is_incomplete);
    }

    #[test]
    fn q_is_typed_and_esc_cancels() {
        let mut component = AttachmentInputPopupComponent::new();

        type_text(&mut component, "q");
        let result = component.process_event(key_event(KeyCode::Esc));

        assert_eq!(component.filename.lines()[0], "q");
        assert!(matches!(result, Some(EventProcessResult::Canceled)));
    }

    #[test]
    fn snapshot_attachment_input_popup_incomplete() {
        let mut component = AttachmentInputPopupComponent::new();
        type_text(&mut component, "screen.png");
        component.process_event(key_event(KeyCode::Tab));
        component.process_event(key_event(KeyCode::Enter));

        render_snapshot(
            "attachment_input_popup_incomplete",
            60,
            14,
            component.create_widget(),
        );
    }
}
