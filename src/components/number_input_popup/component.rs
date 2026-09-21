use ratatui::style::{Modifier, Style};
use ratatui_textarea::{Input, Key, TextArea};

use super::widget::NumberInputPopupWidget;
use crate::inputs::{InputEvent, KeyCode, KeyEvent};

pub enum EventProcessResult {
    Entered,
    Canceled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Confirm,
    Cancel,
    InputKey(KeyEvent),
}

/// 0以上の数値を1つ入力するpopup。空欄での確定は値なし(None)として扱う。
pub struct NumberInputPopupComponent<'a> {
    title: &'static str,
    textarea: TextArea<'a>,
    is_invalid: bool,
    observer: Box<dyn FnMut(Option<f64>) + 'a>,
}

impl<'a> NumberInputPopupComponent<'a> {
    pub fn new(
        title: &'static str,
        value: Option<f64>,
        observer: Box<dyn FnMut(Option<f64>) + 'a>,
    ) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        if let Some(value) = value {
            textarea.insert_str(value.to_string());
        }

        Self {
            title,
            textarea,
            is_invalid: false,
            observer,
        }
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        let InputEvent::Key(key) = event;

        let action = self.interpret_key_event(key)?;
        self.process_action(action)
    }

    pub fn create_widget<'b>(&'b self) -> NumberInputPopupWidget<'b> {
        NumberInputPopupWidget::new(self.title, &self.textarea, self.is_invalid)
    }

    fn interpret_key_event(&self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Enter => Some(Action::Confirm),
            // 数値入力でqは使わないため、SelectBoxPopupと同じく閉じる操作に割り当てる
            KeyCode::Esc | KeyCode::Char('q') => Some(Action::Cancel),
            KeyCode::Char(c) if c.is_ascii_digit() => Some(Action::InputKey(key)),
            // 小数点は整数部の後ろに1つだけ受け付ける
            KeyCode::Char('.')
                if self.textarea.cursor().1 > 0 && !self.textarea.lines()[0].contains('.') =>
            {
                Some(Action::InputKey(key))
            }
            KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End => Some(Action::InputKey(key)),
            _ => None,
        }
    }

    // Componentの入力をplatform非依存に保ち、Textareaへ渡すこの境界でだけ専用型へ変換する。
    fn textarea_input(key: KeyEvent) -> Input {
        let textarea_key = match key.code {
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab | KeyCode::BackTab => Key::Tab,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
        };
        Input {
            key: textarea_key,
            ctrl: key.modifiers.is_control(),
            alt: false,
            shift: key.modifiers.is_shift() || key.code == KeyCode::BackTab,
        }
    }

    fn process_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::Confirm => {
                let Some(value) = parse_value(&self.textarea.lines()[0]) else {
                    self.is_invalid = true;
                    return None;
                };
                (self.observer)(value);
                Some(EventProcessResult::Entered)
            }
            Action::Cancel => Some(EventProcessResult::Canceled),
            Action::InputKey(key) => {
                self.textarea.input(Self::textarea_input(key));
                self.is_invalid = false;
                None
            }
        }
    }
}

/// 空欄は`Some(None)`、有限な数値は`Some(Some(_))`、桁が多すぎて無限大になる値は`None`を返す。
///
/// 入力は数字と`.`1つまでに限られるため、負数や数値以外の文字列は来ない。
fn parse_value(text: &str) -> Option<Option<f64>> {
    if text.is_empty() {
        return Some(None);
    }
    // 末尾の`.`は`0`を補って`1.0`として扱う。`.`だけの場合は`.0`になる
    let text = if text.ends_with('.') {
        format!("{text}0")
    } else {
        text.to_string()
    };
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::inputs::KeyModifiers;
    use crate::test_support::render_snapshot;

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn component_with_observer(
        value: Option<f64>,
        entered: Rc<RefCell<Option<Option<f64>>>>,
    ) -> NumberInputPopupComponent<'static> {
        NumberInputPopupComponent::new(
            "予定工数",
            value,
            Box::new(move |value| {
                *entered.borrow_mut() = Some(value);
            }),
        )
    }

    fn type_text(component: &mut NumberInputPopupComponent<'_>, text: &str) {
        for c in text.chars() {
            component.process_event(key_event(KeyCode::Char(c)));
        }
    }

    #[test]
    fn new_fills_textarea_with_current_value() {
        let entered = Rc::new(RefCell::new(None));
        let component = component_with_observer(Some(1.5), entered);

        assert_eq!(component.textarea.lines()[0], "1.5");
    }

    #[test]
    fn enter_notifies_typed_fractional_value() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered.clone());
        type_text(&mut component, "2.25");

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*entered.borrow(), Some(Some(2.25)));
    }

    #[test]
    fn enter_on_empty_text_notifies_none() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(Some(8.0), entered.clone());
        component.process_event(key_event(KeyCode::Backspace));

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*entered.borrow(), Some(None));
    }

    #[test]
    fn non_numeric_chars_are_ignored() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered);

        type_text(&mut component, "1a-h2");

        assert_eq!(component.textarea.lines()[0], "12");
    }

    #[test]
    fn keys_other_than_editing_keys_are_ignored() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(Some(1.0), entered);

        component.process_event(key_event(KeyCode::Tab));
        component.process_event(key_event(KeyCode::BackTab));
        component.process_event(key_event(KeyCode::Left));
        component.process_event(key_event(KeyCode::Char('5')));

        assert_eq!(component.textarea.lines()[0], "51");
    }

    #[test]
    fn second_decimal_point_is_ignored() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered);

        type_text(&mut component, "1..2.3");

        assert_eq!(component.textarea.lines()[0], "1.23");
    }

    #[test]
    fn leading_decimal_point_is_ignored() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(Some(5.0), entered);

        component.process_event(key_event(KeyCode::Home));
        component.process_event(key_event(KeyCode::Char('.')));

        assert_eq!(component.textarea.lines()[0], "5");
    }

    #[test]
    fn enter_on_trailing_decimal_point_completes_zero() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered.clone());
        type_text(&mut component, "1.");

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*entered.borrow(), Some(Some(1.0)));
    }

    #[test]
    fn enter_on_only_decimal_point_notifies_zero() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered.clone());
        type_text(&mut component, "1.");
        component.process_event(key_event(KeyCode::Home));
        component.process_event(key_event(KeyCode::Delete));

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(*entered.borrow(), Some(Some(0.0)));
    }

    #[test]
    fn enter_on_too_large_value_keeps_popup_open_until_edited() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered.clone());
        type_text(&mut component, &"9".repeat(400));

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(result.is_none());
        assert!(component.is_invalid);
        assert_eq!(*entered.borrow(), None);

        component.process_event(key_event(KeyCode::Backspace));
        assert!(!component.is_invalid);
    }

    #[test]
    fn esc_and_q_cancel_without_notifying() {
        for code in [KeyCode::Esc, KeyCode::Char('q')] {
            let entered = Rc::new(RefCell::new(None));
            let mut component = component_with_observer(Some(1.0), entered.clone());

            let result = component.process_event(key_event(code));

            assert!(matches!(result, Some(EventProcessResult::Canceled)));
            assert_eq!(*entered.borrow(), None);
        }
    }

    #[test]
    fn snapshot_number_input_popup_too_large_value() {
        let entered = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(None, entered);
        type_text(&mut component, &"9".repeat(400));
        component.process_event(key_event(KeyCode::Enter));

        render_snapshot(
            "number_input_popup_too_large_value",
            50,
            12,
            component.create_widget(),
        );
    }
}
