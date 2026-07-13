use crossterm::event::{Event, KeyCode, KeyModifiers};
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

        match key.code {
            KeyCode::Esc => Some(EventProcessResult::Quited),
            KeyCode::Char('j') => {
                if self.input_mode == InputMode::Navigating {
                    self.focus_next();
                }
                None
            }
            KeyCode::Char('k') => {
                if self.input_mode == InputMode::Navigating {
                    self.focus_previous();
                }
                None
            }
            KeyCode::Enter => {
                match self.focused_field {
                    FocusField::Activity => {
                        return Some(EventProcessResult::OpenTimeEntityActivitiesPopup);
                    }
                    FocusField::Hours => {
                        let entering = self.input_mode != InputMode::Editing(EditableField::Hours);
                        self.input_mode = if entering {
                            InputMode::Editing(EditableField::Hours)
                        } else {
                            InputMode::Navigating
                        };
                        Self::set_textarea_cursor(&mut self.hours_textarea, entering);
                    }
                    FocusField::Memo => {
                        let entering = self.input_mode != InputMode::Editing(EditableField::Memo);
                        self.input_mode = if entering {
                            InputMode::Editing(EditableField::Memo)
                        } else {
                            InputMode::Navigating
                        };
                        Self::set_textarea_cursor(&mut self.memo_textarea, entering);
                    }
                    FocusField::Submit => {
                        return Some(EventProcessResult::Submited);
                    }
                }
                None
            }
            KeyCode::Char('c') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Some(EventProcessResult::Quited);
                }
                None
            }
            _ => match self.input_mode {
                InputMode::Editing(EditableField::Hours) => {
                    self.hours_textarea.input(key);
                    None
                }
                InputMode::Editing(EditableField::Memo) => {
                    self.memo_textarea.input(key);
                    None
                }
                InputMode::Navigating => None,
            },
        }
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
