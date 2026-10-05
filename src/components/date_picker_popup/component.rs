use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate, TimeZone};
use ratatui_textarea::TextArea;

use crate::platform::input::{InputEvent, KeyCode, KeyEvent};

use super::widget::DatePickerPopupWidget;

pub enum EventProcessResult {
    Entered,
    Canceled,
    /// popupを開いたまま、イベントを内部で処理した
    Handled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusField {
    Year,
    Month,
    Day,
    CalendarButton,
    Cancel,
    /// 開いている間はhjklなどの操作を閉じるまでカレンダー内に留める
    Calendar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    FocusNext,
    FocusPrevious,
    FocusLeft,
    FocusRight,
    FocusDown,
    FocusUp,
    MoveFocusedDate(i64),
    MoveFocusedMonth(i32),
    Confirm,
    CloseCalendar,
    InputKey(KeyEvent),
}

pub struct DatePickerPopupComponent<'a> {
    year_textarea: TextArea<'a>,
    month_textarea: TextArea<'a>,
    day_textarea: TextArea<'a>,
    display_start: DateTime<Local>,
    focused_date: DateTime<Local>,
    selected_date: Option<DateTime<Local>>,
    focused_field: FocusField,
    observer: Box<dyn FnMut(DateTime<Local>) + 'a>,
}

impl<'a> DatePickerPopupComponent<'a> {
    pub fn new(
        selected_date: Option<DateTime<Local>>,
        observer: Box<dyn FnMut(DateTime<Local>) + 'a>,
    ) -> Self {
        let has_selected_date = selected_date.is_some();
        let input_date = selected_date
            .map(start_of_local_day)
            .unwrap_or_else(|| start_of_local_day(Local::now()));
        let selected_date = has_selected_date.then_some(input_date);
        let focused_date = input_date;

        Self {
            year_textarea: textarea_with_value(&format!("{:04}", input_date.year())),
            month_textarea: textarea_with_value(&format!("{:02}", input_date.month())),
            day_textarea: textarea_with_value(&format!("{:02}", input_date.day())),
            display_start: recent_sunday(input_date),
            focused_date,
            selected_date,
            focused_field: FocusField::Year,
            observer,
        }
    }

    pub fn process_event(&mut self, event: InputEvent) -> Option<EventProcessResult> {
        let InputEvent::Key(key) = event;
        let action = self.interpret_key_event(key)?;
        self.process_action(action)
    }

    pub fn create_widget<'b>(&'b self) -> DatePickerPopupWidget<'b> {
        DatePickerPopupWidget::new(
            &self.year_textarea,
            &self.month_textarea,
            &self.day_textarea,
            self.focused_field == FocusField::Year,
            self.focused_field == FocusField::Month,
            self.focused_field == FocusField::Day,
            self.focused_field == FocusField::CalendarButton,
            self.focused_field == FocusField::Cancel,
            self.display_start,
            if self.focused_field == FocusField::Calendar {
                Some(self.focused_date)
            } else {
                None
            },
            self.selected_date,
        )
    }

    fn interpret_key_event(&self, key: KeyEvent) -> Option<Action> {
        if self.focused_field == FocusField::Calendar {
            return Self::interpret_calendar_key_event(key);
        }
        match key.code {
            KeyCode::Tab => Some(Action::FocusNext),
            KeyCode::BackTab => Some(Action::FocusPrevious),
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Char('h') => Some(Action::FocusLeft),
            KeyCode::Char('l') => Some(Action::FocusRight),
            KeyCode::Char('j') => Some(Action::FocusDown),
            KeyCode::Char('k') => Some(Action::FocusUp),
            _ => match self.focused_field {
                FocusField::Year | FocusField::Month | FocusField::Day => {
                    Some(Action::InputKey(key))
                }
                FocusField::CalendarButton | FocusField::Cancel | FocusField::Calendar => None,
            },
        }
    }

    fn interpret_calendar_key_event(key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Esc | KeyCode::Char('q') => Some(Action::CloseCalendar),
            KeyCode::Char('h') => Some(Action::MoveFocusedDate(-1)),
            KeyCode::Char('l') => Some(Action::MoveFocusedDate(1)),
            KeyCode::Char('j') => Some(Action::MoveFocusedDate(7)),
            KeyCode::Char('k') => Some(Action::MoveFocusedDate(-7)),
            KeyCode::Char('D') if key.modifiers.is_shift() => Some(Action::MoveFocusedMonth(1)),
            KeyCode::Char('U') if key.modifiers.is_shift() => Some(Action::MoveFocusedMonth(-1)),
            _ => None,
        }
    }

    fn process_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::FocusNext => {
                self.focus_next();
                Some(EventProcessResult::Handled)
            }
            Action::FocusPrevious => {
                self.focus_previous();
                Some(EventProcessResult::Handled)
            }
            Action::FocusLeft => {
                self.focus_left();
                Some(EventProcessResult::Handled)
            }
            Action::FocusRight => {
                self.focus_right();
                Some(EventProcessResult::Handled)
            }
            Action::FocusDown => {
                self.focus_down();
                Some(EventProcessResult::Handled)
            }
            Action::FocusUp => {
                self.focus_up();
                Some(EventProcessResult::Handled)
            }
            Action::MoveFocusedDate(days) => {
                self.move_focused_date(days);
                Some(EventProcessResult::Handled)
            }
            Action::MoveFocusedMonth(months) => {
                self.move_focused_month(months);
                Some(EventProcessResult::Handled)
            }
            Action::Confirm => self.confirm(),
            Action::CloseCalendar => {
                self.focused_field = FocusField::CalendarButton;
                Some(EventProcessResult::Handled)
            }
            Action::InputKey(key) => {
                self.focused_textarea_mut().input(Self::textarea_input(key));
                Some(EventProcessResult::Handled)
            }
        }
    }

    // Componentの入力をplatform非依存に保ち、Textareaへ渡すこの境界でだけ専用型へ変換する。
    fn textarea_input(key: KeyEvent) -> ratatui_textarea::Input {
        let textarea_key = match key.code {
            KeyCode::Char(c) => ratatui_textarea::Key::Char(c),
            KeyCode::Enter => ratatui_textarea::Key::Enter,
            KeyCode::Esc => ratatui_textarea::Key::Esc,
            KeyCode::Tab | KeyCode::BackTab => ratatui_textarea::Key::Tab,
            KeyCode::Backspace => ratatui_textarea::Key::Backspace,
            KeyCode::Delete => ratatui_textarea::Key::Delete,
            KeyCode::Left => ratatui_textarea::Key::Left,
            KeyCode::Right => ratatui_textarea::Key::Right,
            KeyCode::Home => ratatui_textarea::Key::Home,
            KeyCode::End => ratatui_textarea::Key::End,
        };
        ratatui_textarea::Input {
            key: textarea_key,
            ctrl: key.modifiers.is_control(),
            alt: false,
            shift: key.modifiers.is_shift() || key.code == KeyCode::BackTab,
        }
    }

    // Calendarは開閉で出入りするため、Tab順に含めない
    fn focus_next(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Year => FocusField::Month,
            FocusField::Month => FocusField::Day,
            FocusField::Day => FocusField::CalendarButton,
            FocusField::CalendarButton => FocusField::Cancel,
            field => field,
        };
    }

    fn focus_previous(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Month => FocusField::Year,
            FocusField::Day => FocusField::Month,
            FocusField::CalendarButton => FocusField::Day,
            FocusField::Cancel => FocusField::CalendarButton,
            field => field,
        };
    }

    fn focus_left(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Month => FocusField::Year,
            FocusField::Day => FocusField::Month,
            FocusField::Cancel => FocusField::CalendarButton,
            field => field,
        };
    }

    fn focus_right(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Year => FocusField::Month,
            FocusField::Month => FocusField::Day,
            FocusField::CalendarButton => FocusField::Cancel,
            field => field,
        };
    }

    fn focus_down(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Year | FocusField::Month | FocusField::Day => FocusField::CalendarButton,
            field => field,
        };
    }

    fn focus_up(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::CalendarButton | FocusField::Cancel => FocusField::Year,
            field => field,
        };
    }

    fn open_calendar(&mut self) {
        self.focused_field = FocusField::Calendar;
        // 入力欄が日付として不正な場合は、前回カレンダーで見ていた日付から始める
        if let Some(input_date) = self.input_date() {
            self.focused_date = input_date;
            self.display_start = recent_sunday(input_date);
        }
    }

    fn apply_calendar_date(&mut self) {
        let date = self.focused_date;
        self.year_textarea = textarea_with_value(&format!("{:04}", date.year()));
        self.month_textarea = textarea_with_value(&format!("{:02}", date.month()));
        self.day_textarea = textarea_with_value(&format!("{:02}", date.day()));
        self.focused_field = FocusField::CalendarButton;
    }

    fn move_focused_date(&mut self, days: i64) {
        self.focused_date = if days >= 0 {
            self.focused_date
                .checked_add_days(Days::new(days as u64))
                .unwrap_or(self.focused_date)
        } else {
            self.focused_date
                .checked_sub_days(Days::new(days.unsigned_abs()))
                .unwrap_or(self.focused_date)
        };
        self.display_start = recent_sunday(self.focused_date);
    }

    fn move_focused_month(&mut self, months: i32) {
        let moved = if months >= 0 {
            self.focused_date
                .checked_add_months(Months::new(months as u32))
        } else {
            self.focused_date
                .checked_sub_months(Months::new(months.unsigned_abs()))
        };

        if let Some(moved) = moved {
            self.focused_date = moved;
            self.display_start = recent_sunday(moved);
        }
    }

    fn confirm(&mut self) -> Option<EventProcessResult> {
        match self.focused_field {
            FocusField::Year | FocusField::Month | FocusField::Day => {
                let Some(date) = self.input_date() else {
                    return Some(EventProcessResult::Handled);
                };
                self.selected_date = Some(date);
                (self.observer)(date);
                Some(EventProcessResult::Entered)
            }
            FocusField::CalendarButton => {
                self.open_calendar();
                Some(EventProcessResult::Handled)
            }
            FocusField::Calendar => {
                self.apply_calendar_date();
                Some(EventProcessResult::Handled)
            }
            FocusField::Cancel => Some(EventProcessResult::Canceled),
        }
    }

    fn focused_textarea_mut(&mut self) -> &mut TextArea<'a> {
        match self.focused_field {
            FocusField::Year => &mut self.year_textarea,
            FocusField::Month => &mut self.month_textarea,
            FocusField::Day => &mut self.day_textarea,
            FocusField::CalendarButton | FocusField::Cancel | FocusField::Calendar => {
                unreachable!("only date input fields accept text input")
            }
        }
    }

    fn input_date(&self) -> Option<DateTime<Local>> {
        let year = textarea_text(&self.year_textarea).parse::<i32>().ok()?;
        let month = textarea_text(&self.month_textarea).parse::<u32>().ok()?;
        let day = textarea_text(&self.day_textarea).parse::<u32>().ok()?;
        local_date(year, month, day)
    }
}

fn textarea_with_value(value: &str) -> TextArea<'static> {
    let mut textarea = TextArea::default();
    textarea.insert_str(value);
    textarea
}

fn textarea_text(textarea: &TextArea<'_>) -> String {
    textarea.lines().join("")
}

fn start_of_local_day(date: DateTime<Local>) -> DateTime<Local> {
    local_date(date.year(), date.month(), date.day()).expect("existing DateTime has a valid date")
}

fn recent_sunday(date: DateTime<Local>) -> DateTime<Local> {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_sunday() as u64))
        .unwrap_or(date)
}

fn local_date(year: i32, month: u32, day: u32) -> Option<DateTime<Local>> {
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
        .single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};
    use crate::test_support::local_datetime;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, modifiers))
    }

    fn component_with_observer(
        selected: Rc<RefCell<Option<chrono::DateTime<chrono::Local>>>>,
    ) -> DatePickerPopupComponent<'static> {
        DatePickerPopupComponent::new(
            Some(local_datetime("2026-02-16T00:00:00+09:00")),
            Box::new(move |date| {
                *selected.borrow_mut() = Some(date);
            }),
        )
    }

    #[test]
    fn new_sets_display_start_to_selected_dates_recent_sunday() {
        let selected = Rc::new(RefCell::new(None));

        let component = component_with_observer(selected);

        assert_eq!(
            component.display_start,
            local_datetime("2026-02-15T00:00:00+09:00")
        );
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-16T00:00:00+09:00")
        );
    }

    #[test]
    fn new_without_selected_date_uses_todays_recent_sunday() {
        let selected: Rc<RefCell<Option<chrono::DateTime<chrono::Local>>>> =
            Rc::new(RefCell::new(None));
        let before_today = start_of_local_day(Local::now());

        let component = DatePickerPopupComponent::new(
            None,
            Box::new(move |date| {
                *selected.borrow_mut() = Some(date);
            }),
        );
        let after_today = start_of_local_day(Local::now());

        assert!(
            component.focused_date == before_today || component.focused_date == after_today,
            "focused_date should be today's local date"
        );
        assert_eq!(
            component.display_start,
            recent_sunday(component.focused_date)
        );
    }

    #[test]
    fn tab_and_shift_tab_move_focus_in_widget_order_without_calendar() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);

        assert_eq!(component.focused_field, FocusField::Year);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Month);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Day);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::CalendarButton);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Cancel);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Cancel);

        component.process_event(key_event(KeyCode::BackTab));
        assert_eq!(component.focused_field, FocusField::CalendarButton);
        component.process_event(key_event(KeyCode::BackTab));
        assert_eq!(component.focused_field, FocusField::Day);
        component.process_event(key_event(KeyCode::BackTab));
        assert_eq!(component.focused_field, FocusField::Month);
        component.process_event(key_event(KeyCode::BackTab));
        assert_eq!(component.focused_field, FocusField::Year);
    }

    #[test]
    fn h_and_l_move_focus_between_date_inputs() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);

        component.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(component.focused_field, FocusField::Month);
        component.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(component.focused_field, FocusField::Day);
        component.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(component.focused_field, FocusField::Month);
        component.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(component.focused_field, FocusField::Year);
    }

    #[test]
    fn h_and_l_move_focus_between_buttons() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);
        component.focused_field = FocusField::CalendarButton;

        component.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(component.focused_field, FocusField::Cancel);
        component.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(component.focused_field, FocusField::CalendarButton);
    }

    #[test]
    fn j_moves_from_each_date_input_to_calendar_button() {
        for field in [FocusField::Year, FocusField::Month, FocusField::Day] {
            let selected = Rc::new(RefCell::new(None));
            let mut component = component_with_observer(selected);
            component.focused_field = field;

            component.process_event(key_event(KeyCode::Char('j')));

            assert_eq!(component.focused_field, FocusField::CalendarButton);
        }
    }

    #[test]
    fn k_moves_from_each_button_to_year_input() {
        for field in [FocusField::CalendarButton, FocusField::Cancel] {
            let selected = Rc::new(RefCell::new(None));
            let mut component = component_with_observer(selected);
            component.focused_field = field;

            component.process_event(key_event(KeyCode::Char('k')));

            assert_eq!(component.focused_field, FocusField::Year);
        }
    }

    #[test]
    fn enter_on_calendar_button_opens_calendar_at_input_date() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.day_textarea.delete_line_by_head();
        component.day_textarea.insert_str("25");
        component.focused_field = FocusField::CalendarButton;

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert_eq!(component.focused_field, FocusField::Calendar);
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-25T00:00:00+09:00")
        );
        assert_eq!(
            component.display_start,
            local_datetime("2026-02-22T00:00:00+09:00")
        );
        assert_eq!(*selected.borrow(), None);
    }

    #[test]
    fn calendar_keeps_focus_on_tab_and_shift_tab() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);
        component.focused_field = FocusField::Calendar;

        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Calendar);
        component.process_event(key_event(KeyCode::BackTab));
        assert_eq!(component.focused_field, FocusField::Calendar);
    }

    #[test]
    fn calendar_h_l_j_k_move_focused_date() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);
        component.focused_field = FocusField::Calendar;

        component.process_event(key_event(KeyCode::Char('l')));
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-17T00:00:00+09:00")
        );
        assert_eq!(
            component.display_start,
            local_datetime("2026-02-15T00:00:00+09:00")
        );
        component.process_event(key_event(KeyCode::Char('h')));
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-16T00:00:00+09:00")
        );
        assert_eq!(
            component.display_start,
            local_datetime("2026-02-15T00:00:00+09:00")
        );
        component.process_event(key_event(KeyCode::Char('j')));
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-23T00:00:00+09:00")
        );
        assert_eq!(
            component.display_start,
            local_datetime("2026-02-22T00:00:00+09:00")
        );
        component.process_event(key_event(KeyCode::Char('k')));
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-16T00:00:00+09:00")
        );
        assert_eq!(
            component.display_start,
            local_datetime("2026-02-15T00:00:00+09:00")
        );
    }

    #[test]
    fn calendar_shift_d_and_shift_u_move_display_month() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);
        component.focused_field = FocusField::Calendar;

        component.process_event(key_event_with_modifiers(
            KeyCode::Char('D'),
            KeyModifiers::shift(),
        ));
        assert_eq!(
            component.display_start,
            local_datetime("2026-03-15T00:00:00+09:00")
        );
        assert_eq!(
            component.focused_date,
            local_datetime("2026-03-16T00:00:00+09:00")
        );

        component.process_event(key_event_with_modifiers(
            KeyCode::Char('U'),
            KeyModifiers::shift(),
        ));
        assert_eq!(
            component.display_start,
            local_datetime("2026-02-15T00:00:00+09:00")
        );
        assert_eq!(
            component.focused_date,
            local_datetime("2026-02-16T00:00:00+09:00")
        );
    }

    #[test]
    fn enter_on_calendar_writes_focused_date_to_inputs_and_closes_calendar() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.focused_field = FocusField::Calendar;
        component.process_event(key_event_with_modifiers(
            KeyCode::Char('D'),
            KeyModifiers::shift(),
        ));
        component.process_event(key_event(KeyCode::Char('l')));

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert_eq!(component.focused_field, FocusField::CalendarButton);
        assert_eq!(textarea_text(&component.year_textarea), "2026");
        assert_eq!(textarea_text(&component.month_textarea), "03");
        assert_eq!(textarea_text(&component.day_textarea), "17");
        assert_eq!(*selected.borrow(), None);
    }

    #[test]
    fn q_and_esc_on_calendar_discard_focused_date_and_close_calendar() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let selected = Rc::new(RefCell::new(None));
            let mut component = component_with_observer(selected.clone());
            component.focused_field = FocusField::Calendar;
            component.process_event(key_event(KeyCode::Char('l')));

            let result = component.process_event(key_event(code));

            assert!(matches!(result, Some(EventProcessResult::Handled)));
            assert_eq!(component.focused_field, FocusField::CalendarButton);
            assert_eq!(textarea_text(&component.year_textarea), "2026");
            assert_eq!(textarea_text(&component.month_textarea), "02");
            assert_eq!(textarea_text(&component.day_textarea), "16");
            assert_eq!(*selected.borrow(), None);
        }
    }

    #[test]
    fn enter_on_valid_date_inputs_notifies_observer_with_input_date() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.year_textarea.delete_line_by_head();
        component.year_textarea.insert_str("2026");
        component.month_textarea.delete_line_by_head();
        component.month_textarea.insert_str("04");
        component.day_textarea.delete_line_by_head();
        component.day_textarea.insert_str("30");

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(
            *selected.borrow(),
            Some(local_datetime("2026-04-30T00:00:00+09:00"))
        );
    }

    #[test]
    fn enter_on_invalid_date_inputs_does_not_notify_observer() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.month_textarea.delete_line_by_head();
        component.month_textarea.insert_str("02");
        component.day_textarea.delete_line_by_head();
        component.day_textarea.insert_str("30");

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert_eq!(*selected.borrow(), None);
    }

    #[test]
    fn enter_on_cancel_does_not_notify_observer() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.focused_field = FocusField::Cancel;

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Canceled)));
        assert_eq!(*selected.borrow(), None);
    }

    #[test]
    fn q_on_year_field_is_input_and_handled() {
        let mut component = DatePickerPopupComponent::new(None, Box::new(|_| {}));

        let result = component.process_event(key_event(KeyCode::Char('q')));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
        assert!(textarea_text(&component.year_textarea).contains('q'));
    }

    #[test]
    fn focus_move_returns_handled() {
        let mut component = DatePickerPopupComponent::new(None, Box::new(|_| {}));

        let result = component.process_event(key_event(KeyCode::Tab));

        assert!(matches!(result, Some(EventProcessResult::Handled)));
    }

    #[test]
    fn create_widget_reflects_cancel_focus() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);
        component.focused_field = FocusField::Cancel;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 24)).unwrap();

        terminal
            .draw(|frame| frame.render_widget(component.create_widget(), frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let popup_area = DatePickerPopupWidget::popup_area(buffer.area, false);
        let button_y = popup_area.y + popup_area.height - 4;
        assert_eq!(
            buffer[(popup_area.x + 1, button_y)].fg,
            ratatui::style::Color::Reset
        );
        assert_eq!(
            buffer[(popup_area.x + 13, button_y)].fg,
            ratatui::style::Color::LightGreen
        );
    }
}
