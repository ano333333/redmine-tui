use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::Position;
use ratatui::style::{Modifier, Style};
use ratatui_textarea::TextArea;

use crate::app::Store;
use crate::entities::TimeEntityActivityId;

use super::widget::SpentTimeInputPopupWidget;

pub enum EventProcessResult {
    Submited,
    Quited,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Activity,
    // boolは入力中か否かを表す
    Hours(bool),
    // boolは入力中か否かを表す
    Memo(bool),
    Submit,
}

pub struct SpentTimeInputPopupComponent {
    activity_id: TimeEntityActivityId,
    // AppComponentがこのstructを保持するためTextareaを'statisで持つ。
    // enum PopupComponentの定義を参照。
    hours_textarea: TextArea<'static>,
    memo_textarea: TextArea<'static>,
    focused_target: FocusTarget,
}

impl SpentTimeInputPopupComponent {
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
            focused_target: FocusTarget::Activity,
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };

        match key.code {
            KeyCode::Esc => Some(EventProcessResult::Quited),
            KeyCode::Char('j') => {
                if self.focused_target != FocusTarget::Hours(true)
                    && self.focused_target != FocusTarget::Memo(true)
                {
                    self.focus_next();
                }
                None
            }
            KeyCode::Char('k') => {
                if self.focused_target != FocusTarget::Hours(true)
                    && self.focused_target != FocusTarget::Memo(true)
                {
                    self.focus_previous();
                }
                None
            }
            KeyCode::Enter => {
                match self.focused_target {
                    FocusTarget::Activity => {
                        // FIXME: `self.focused_target == FocusTarget::Activity`の場合TimeActivityEntityの選択ポップアップを重ねる
                        self.focus_next();
                    }
                    FocusTarget::Hours(entering) => {
                        self.focused_target = FocusTarget::Hours(!entering);
                        Self::set_textarea_cursor(&mut self.hours_textarea, !entering);
                    }
                    FocusTarget::Memo(entering) => {
                        self.focused_target = FocusTarget::Memo(!entering);
                        Self::set_textarea_cursor(&mut self.memo_textarea, !entering);
                    }
                    FocusTarget::Submit => {
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
            _ => match self.focused_target {
                FocusTarget::Hours(entering) => {
                    if entering {
                        self.hours_textarea.input(key);
                    }
                    None
                }
                FocusTarget::Memo(entering) => {
                    if entering {
                        self.memo_textarea.input(key);
                    }
                    None
                }
                _ => None,
            },
        }
    }

    pub fn create_widget<'b>(&'b self, store: &'b Store) -> SpentTimeInputPopupWidget<'b, 'static> {
        let activity = store.get_time_entity_activities().get(&self.activity_id);
        SpentTimeInputPopupWidget::new(
            activity.map(|act| act.name.as_str()).unwrap_or(""),
            &self.hours_textarea,
            &self.memo_textarea,
            self.focused_target == FocusTarget::Activity,
            self.focused_target == FocusTarget::Hours(true)
                || self.focused_target == FocusTarget::Hours(false),
            self.focused_target == FocusTarget::Memo(true)
                || self.focused_target == FocusTarget::Memo(false),
            self.focused_target == FocusTarget::Submit,
        )
    }

    /// クライアント領域全体に対するカーソル位置
    ///
    /// * `area` - クライアント領域
    pub fn cursor_position(&self, area: ratatui::layout::Rect) -> Option<Position> {
        let popup_area = SpentTimeInputPopupWidget::popup_area(area);
        match self.focused_target {
            FocusTarget::Activity => Some(Position {
                x: popup_area.x + 2,
                y: popup_area.y + 2,
            }),
            // ratatui_textarea::TextAreaが表示するカーソルをそのまま使用する
            _ => None,
        }
    }

    fn focus_next(&mut self) {
        match self.focused_target {
            FocusTarget::Activity => {
                self.focused_target = FocusTarget::Hours(false);
            }
            FocusTarget::Hours(_) => {
                Self::set_textarea_cursor(&mut self.hours_textarea, false);
                self.focused_target = FocusTarget::Memo(false);
            }
            FocusTarget::Memo(_) => {
                Self::set_textarea_cursor(&mut self.memo_textarea, false);
                self.focused_target = FocusTarget::Submit;
            }
            FocusTarget::Submit => {}
        }
    }

    fn focus_previous(&mut self) {
        match self.focused_target {
            FocusTarget::Activity => {}
            FocusTarget::Hours(_) => {
                Self::set_textarea_cursor(&mut self.hours_textarea, false);
                self.focused_target = FocusTarget::Activity;
            }
            FocusTarget::Memo(_) => {
                Self::set_textarea_cursor(&mut self.memo_textarea, false);
                self.focused_target = FocusTarget::Hours(false);
            }
            FocusTarget::Submit => {
                self.focused_target = FocusTarget::Memo(false);
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
