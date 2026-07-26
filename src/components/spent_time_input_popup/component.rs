use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Position;
use ratatui::style::{Modifier, Style};
use ratatui_textarea::TextArea;

use crate::app::Store;
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
    FocusNext,
    FocusPrevious,
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
            KeyCode::Char('j') if self.input_mode == InputMode::Navigating => {
                Some(Action::FocusNext)
            }
            KeyCode::Char('k') if self.input_mode == InputMode::Navigating => {
                Some(Action::FocusPrevious)
            }
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::Quit)
            }
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
            Some(Action::FocusNext) => {
                self.focus_next();
                None
            }
            Some(Action::FocusPrevious) => {
                self.focus_previous();
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
                        self.hours_textarea.input(key);
                    }
                    InputMode::Editing(EditableField::Memo) => {
                        self.memo_textarea.input(key);
                    }
                    InputMode::Navigating => {}
                }
                None
            }
            None => None,
        }
    }

    fn focus_next(&mut self) {
        self.input_mode = InputMode::Navigating;
        match self.focused_field {
            FocusField::Activity => {
                self.focused_field = FocusField::Hours;
            }
            FocusField::Hours => {
                Self::set_textarea_cursor(&mut self.hours_textarea, false);
                self.focused_field = FocusField::Memo;
            }
            FocusField::Memo => {
                Self::set_textarea_cursor(&mut self.memo_textarea, false);
                self.focused_field = FocusField::Submit;
            }
            FocusField::Submit => {}
        }
    }

    fn focus_previous(&mut self) {
        self.input_mode = InputMode::Navigating;
        match self.focused_field {
            FocusField::Activity => {}
            FocusField::Hours => {
                Self::set_textarea_cursor(&mut self.hours_textarea, false);
                self.focused_field = FocusField::Activity;
            }
            FocusField::Memo => {
                Self::set_textarea_cursor(&mut self.memo_textarea, false);
                self.focused_field = FocusField::Hours;
            }
            FocusField::Submit => {
                self.focused_field = FocusField::Memo;
            }
        }
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
    use crate::app::Action;
    use crossterm::event::KeyEvent;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    fn store_with_time_entity_activities() -> Store {
        let mut store = Store::new();
        store.consume_action(Action::LoadTimeEntityActivities);
        store
    }

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
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

        let result = component.process_event(key_event(KeyCode::Enter));

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

        assert_eq!(position, Some(Position::new(17, 7)));
    }

    #[test]
    fn j_and_k_move_focus_while_navigating() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Submit);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Submit);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Activity);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Activity);
    }

    #[test]
    fn enter_on_hours_toggles_edit_mode_and_inputs_text() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);
        component.process_event(key_event(KeyCode::Char('j')));

        assert!(component.process_event(key_event(KeyCode::Enter)).is_none());
        assert_eq!(component.focused_field, FocusField::Hours);
        assert_eq!(
            component.input_mode,
            InputMode::Editing(EditableField::Hours)
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('1')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);
        assert_eq!(component.hours_textarea.lines()[0], "1");

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Hours);
        assert_eq!(component.hours_textarea.lines()[0], "1j");

        assert!(component.process_event(key_event(KeyCode::Enter)).is_none());
        assert_eq!(component.input_mode, InputMode::Navigating);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_field, FocusField::Memo);
    }

    #[test]
    fn enter_on_memo_toggles_edit_mode_and_inputs_text() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));

        assert!(component.process_event(key_event(KeyCode::Enter)).is_none());
        assert_eq!(component.focused_field, FocusField::Memo);
        assert_eq!(
            component.input_mode,
            InputMode::Editing(EditableField::Memo)
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('a')))
                .is_none()
        );
        assert_eq!(component.memo_textarea.lines()[0], "a");

        assert!(component.process_event(key_event(KeyCode::Enter)).is_none());
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn enter_on_submit_returns_submitted() {
        let store = store_with_time_entity_activities();
        let mut component = SpentTimeInputPopupComponent::new(&store);
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Submited)));
        assert_eq!(component.focused_field, FocusField::Submit);
        assert_eq!(component.input_mode, InputMode::Navigating);
    }

    #[test]
    fn esc_and_ctrl_c_quit() {
        let store = store_with_time_entity_activities();

        let mut component = SpentTimeInputPopupComponent::new(&store);
        let esc_result = component.process_event(key_event(KeyCode::Esc));
        assert!(matches!(esc_result, Some(EventProcessResult::Quited)));

        let mut component = SpentTimeInputPopupComponent::new(&store);
        let ctrl_c_result = component.process_event(key_event_with_modifiers(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert!(matches!(ctrl_c_result, Some(EventProcessResult::Quited)));
    }
}
