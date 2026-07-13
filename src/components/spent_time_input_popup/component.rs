use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Position;
use ratatui::style::{Modifier, Style};
use ratatui_textarea::TextArea;

use crate::app::Store;
use crate::entities::TimeEntityActivityId;

use super::widget::SpentTimeInputPopupWidget;

pub enum EventProcessResult {
    Submited,
    OpenTimeEntityActivitiesPopup,
    Quited,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusField {
    Activity,
    Hours,
    Memo,
    Submit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Navigating,
    Editing(EditableField),
}

#[derive(Clone, Copy, PartialEq, Eq)]
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
