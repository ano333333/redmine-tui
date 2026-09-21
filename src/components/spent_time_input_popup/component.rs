use crossterm::event::Event;
use ratatui::layout::Position;
use ratatui::style::{Modifier, Style};
use ratatui_textarea::{Input, Key, TextArea};

use crate::inputs::native::convert_key;
use crate::inputs::{InputEvent, KeyCode, KeyEvent};
use crate::stores::Store;
use crate::vos::TimeEntityActivityId;

use super::widget::SpentTimeInputPopupWidget;

pub enum EventProcessResult {
    Submited,
    OpenTimeEntityActivitiesPopup,
    Quited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusField {
    Activity,
    Hours,
    Memo,
    Submit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Navigating,
    Editing(EditableField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditableField {
    Hours,
    Memo,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Quit,
    Focus(FocusField),
    Confirm,
    InputKey(KeyEvent),
}

pub struct SpentTimeInputPopupComponent<'a> {
    activity_id: TimeEntityActivityId,
    // AppComponentがこのstructを保持するためTextareaを'statisで持つ。
    // enum PopupComponentの定義を参照。
    hours_textarea: TextArea<'a>,
    memo_textarea: TextArea<'a>,
    focused_field: FocusField,
    input_mode: InputMode,
}

impl<'a> SpentTimeInputPopupComponent<'a> {
    pub fn new(store: &Store) -> Self {
        let mut hours_textarea = TextArea::default();
        hours_textarea.set_cursor_line_style(Default::default());
        hours_textarea.set_placeholder_text("工数を入力");
        Self::set_textarea_cursor(&mut hours_textarea, false);

        let mut memo_textarea = TextArea::default();
        memo_textarea.set_cursor_line_style(Default::default());
        memo_textarea.set_placeholder_text("メモを入力");
        Self::set_textarea_cursor(&mut memo_textarea, false);

        let activities = store.get_time_entity_activities();
        let activity_id = activities
            .iter()
            .find(|(_, act)| act.is_default)
            .or(activities.iter().next())
            .unwrap()
            .0;

        Self {
            activity_id: *activity_id,
            hours_textarea,
            memo_textarea,
            focused_field: FocusField::Activity,
            input_mode: InputMode::Navigating,
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };
        self.process_input_event(convert_key(key)?)
    }

    fn process_input_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        let InputEvent::Key(key) = event;
        let action = self.interpret_key_event(key);
        self.process_action(action)
    }

    pub fn create_widget<'b>(&'b self, store: &'b Store) -> SpentTimeInputPopupWidget<'b> {
        let activity = store.get_time_entity_activities().get(&self.activity_id);
        SpentTimeInputPopupWidget::new(
            activity.map(|act| act.name.as_str()).unwrap_or(""),
            &self.hours_textarea,
            &self.memo_textarea,
            self.focused_field == FocusField::Activity,
            self.focused_field == FocusField::Hours,
            self.focused_field == FocusField::Memo,
            self.focused_field == FocusField::Submit,
        )
    }

    /// クライアント領域全体に対するカーソル位置
    ///
    /// * `area` - クライアント領域
    pub fn cursor_position(&self, area: ratatui::layout::Rect) -> Option<Position> {
        let popup_area = SpentTimeInputPopupWidget::popup_area(area);
        match self.focused_field {
            FocusField::Activity => Some(Position {
                x: popup_area.x + 2,
                y: popup_area.y + 2,
            }),
            // ratatui_textarea::TextAreaが表示するカーソルをそのまま使用する
            _ => None,
        }
    }

    pub fn on_time_entity_activity_selected(&mut self, id: TimeEntityActivityId) {
        self.activity_id = id;
    }

    fn interpret_key_event(&self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => Some(Action::Quit),
            KeyCode::Char('l')
                if self.input_mode == InputMode::Navigating
                    && self.focused_field == FocusField::Activity =>
            {
                Some(Action::Focus(FocusField::Hours))
            }
            KeyCode::Char('h')
                if self.input_mode == InputMode::Navigating
                    && self.focused_field == FocusField::Hours =>
            {
                Some(Action::Focus(FocusField::Activity))
            }
            KeyCode::Char('j') if self.input_mode == InputMode::Navigating => {
                match self.focused_field {
                    FocusField::Activity | FocusField::Hours => {
                        Some(Action::Focus(FocusField::Memo))
                    }
                    FocusField::Memo => Some(Action::Focus(FocusField::Submit)),
                    FocusField::Submit => None,
                }
            }
            KeyCode::Char('k') if self.input_mode == InputMode::Navigating => {
                match self.focused_field {
                    FocusField::Memo => Some(Action::Focus(FocusField::Activity)),
                    FocusField::Submit => Some(Action::Focus(FocusField::Memo)),
                    FocusField::Activity | FocusField::Hours => None,
                }
            }
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Char('c') if key.modifiers.is_control() => Some(Action::Quit),
            _ => match self.input_mode {
                InputMode::Editing(EditableField::Hours | EditableField::Memo) => {
                    Some(Action::InputKey(key))
                }
                InputMode::Navigating => None,
            },
        }
    }

    fn process_action(&mut self, action: Option<Action>) -> Option<EventProcessResult> {
        match action {
            Some(Action::Quit) => Some(EventProcessResult::Quited),
            Some(Action::Focus(field)) => {
                self.focus(field);
                None
            }
            Some(Action::Confirm) => match self.focused_field {
                FocusField::Activity => Some(EventProcessResult::OpenTimeEntityActivitiesPopup),
                FocusField::Hours => {
                    self.toggle_input_mode(EditableField::Hours);
                    None
                }
                FocusField::Memo => {
                    self.toggle_input_mode(EditableField::Memo);
                    None
                }
                FocusField::Submit => Some(EventProcessResult::Submited),
            },
            Some(Action::InputKey(key)) => {
                match self.input_mode {
                    InputMode::Editing(EditableField::Hours) => {
                        self.hours_textarea.input(Self::textarea_input(key));
                    }
                    InputMode::Editing(EditableField::Memo) => {
                        self.memo_textarea.input(Self::textarea_input(key));
                    }
                    InputMode::Navigating => {}
                }
                None
            }
            None => None,
        }
    }

    // Componentの入力をplatform非依存に保ち、Textareaへ渡すこの境界でだけ専用型へ変換する。
    fn textarea_input(key: KeyEvent) -> Input {
        let textarea_key = match key.code {
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        };
        Input {
            key: textarea_key,
            ctrl: key.modifiers.is_control(),
            alt: false,
            shift: key.modifiers.is_shift() || key.code == KeyCode::BackTab,
        }
    }

    fn focus(&mut self, field: FocusField) {
        self.input_mode = InputMode::Navigating;
        if self.focused_field == FocusField::Hours {
            Self::set_textarea_cursor(&mut self.hours_textarea, false);
        }
        if self.focused_field == FocusField::Memo {
            Self::set_textarea_cursor(&mut self.memo_textarea, false);
        }
        self.focused_field = field;
    }

    fn toggle_input_mode(&mut self, field: EditableField) {
        let entering = self.input_mode != InputMode::Editing(field);
        self.input_mode = if entering {
            InputMode::Editing(field)
        } else {
            InputMode::Navigating
        };
        match field {
            EditableField::Hours => Self::set_textarea_cursor(&mut self.hours_textarea, entering),
            EditableField::Memo => Self::set_textarea_cursor(&mut self.memo_textarea, entering),
        }
    }

    /// `ratatui_textarea::TextArea`に入力中か否かに合わせてcursor/cursor_line styleを設定する
    fn set_textarea_cursor(textarea: &mut TextArea, entering: bool) {
        textarea.set_cursor_style(Self::cursor_style(entering));
        textarea.set_cursor_line_style(Self::cursor_line_style(entering));
    }

    /// `ratatui_textarea::TextArea`の`set_cursor_style()`に指定する`Style`
    fn cursor_style(entering: bool) -> Style {
        if entering {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        }
    }

    /// `ratatui_textarea::TextArea`の`set_cursor_line_style()`に指定する`Style`
    fn cursor_line_style(entering: bool) -> Style {
        if entering {
            Style::default().add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inputs::KeyModifiers;
    use crate::test_support::sync_fixture_entities;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    fn store_with_time_entity_activities() -> Store {
        let mut store = Store::new();
        sync_fixture_entities(&mut store);
        store
    }

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, modifiers))
    }

    fn render_widget_text(
        widget: SpentTimeInputPopupWidget<'_>,
        width: u16,
        height: u16,
    ) -> Vec<String> {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect()
    }

    #[test]
    fn new_selects_default_activity_and_focuses_activity() {
        let store = store_with_time_entity_activities();
        let default_activity_id = store
            .get_time_entity_activities()
            .iter()
            .find(|(_, activity)| activity.is_default)
            .or(store.get_time_entity_activities().iter().next())
            .map(|(id, _)| *id)
            .unwrap();

        let component = SpentTimeInputPopupComponent::new(&store);

        assert_eq!(component.activity_id, default_activity_id);
        assert_eq!(component.focused_field, FocusField::Activity);
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn enter_on_activity_requests_open_time_entity_activities_popup() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);

        let result = component.process_input_event(key_event(KeyCode::Enter));

        assert!(matches!(
            result,
            Some(EventProcessResult::OpenTimeEntityActivitiesPopup)
        ));
        assert_eq!(component.focused_field, FocusField::Activity);
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn process_event_returns_none_for_non_key_event() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);

        let result = component.process_event(Event::Resize(80, 24));

        assert!(result.is_none());
        assert_eq!(component.focused_field, FocusField::Activity);
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn create_widget_renders_selected_activity_name() {
        let store = store_with_time_entity_activities();
        let component = SpentTimeInputPopupComponent::new(&store);

        let lines = render_widget_text(component.create_widget(&store), 60, 20);
        assert_eq!(lines.len(), 20);
        assert!(lines.iter().any(|line| !line.trim().is_empty()));
    }

    #[test]
    fn cursor_position_points_to_activity_when_activity_is_focused() {
        let store = store_with_time_entity_activities();
        let component = SpentTimeInputPopupComponent::new(&store);
        let area = Rect::new(0, 0, 60, 20);

        let position = component.cursor_position(area);

        assert_eq!(position, Some(Position::new(8, 6)));
    }

    #[test]
    fn navigation_keys_move_focus_while_navigating() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('l')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Submit);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Submit);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Activity);

        component.process_input_event(key_event(KeyCode::Char('l')));
        assert_eq!(component.focused_field, FocusField::Hours);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('h')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Activity);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Activity);
    }

    #[test]
    fn enter_on_hours_toggles_edit_mode_and_inputs_text() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);
        component.process_input_event(key_event(KeyCode::Char('l')));

        assert!(
            component
                .process_input_event(key_event(KeyCode::Enter))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);
        assert_eq!(
            component.input_mode,
            InputMode::Editing(EditableField::Hours)
        );

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('1')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);
        assert_eq!(component.hours_textarea.lines()[0], "1");

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);
        assert_eq!(component.hours_textarea.lines()[0], "1j");

        assert!(
            component
                .process_input_event(key_event(KeyCode::Enter))
                .is_none()
        );
        assert_eq!(component.input_mode, InputMode::Navigating);

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);
    }

    #[test]
    fn enter_on_memo_toggles_edit_mode_and_inputs_text() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);
        component.process_input_event(key_event(KeyCode::Char('j')));

        assert!(
            component
                .process_input_event(key_event(KeyCode::Enter))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);
        assert_eq!(
            component.input_mode,
            InputMode::Editing(EditableField::Memo)
        );

        assert!(
            component
                .process_input_event(key_event(KeyCode::Char('a')))
                .is_none()
        );
        assert_eq!(component.memo_textarea.lines()[0], "a");

        assert!(
            component
                .process_input_event(key_event(KeyCode::Enter))
                .is_none()
        );
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn enter_on_submit_returns_submitted() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);
        component.process_input_event(key_event(KeyCode::Char('j')));
        component.process_input_event(key_event(KeyCode::Char('j')));
        component.process_input_event(key_event(KeyCode::Char('j')));

        let result = component.process_input_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Submited)));
        assert_eq!(component.focused_field, FocusField::Submit);
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn esc_and_ctrl_c_quit() {
        let store = store_with_time_entity_activities();

        let mut component = SpentTimeInputPopupComponent::new(&store);
        let esc_result = component.process_input_event(key_event(KeyCode::Esc));
        assert!(matches!(esc_result, Some(EventProcessResult::Quited)));

        let mut component = SpentTimeInputPopupComponent::new(&store);
        let ctrl_c_result = component.process_input_event(key_event_with_modifiers(
            KeyCode::Char('c'),
            KeyModifiers::control(),
        ));
        assert!(matches!(ctrl_c_result, Some(EventProcessResult::Quited)));
    }
}
