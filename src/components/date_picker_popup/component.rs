use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate, TimeZone};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::TextArea;

use super::widget::DatePickerPopupWidget;

pub enum EventProcessResult {
    Entered,
    Canceled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusField {
    Year,
    Month,
    Day,
    Calendar,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    FocusNext,
    FocusPrevious,
    FocusLeft,
    FocusRight,
    FocusCalendar,
    MoveFocusedDate(i64),
    MoveFocusedMonth(i32),
    Confirm,
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
    observer: Box<dyn FnMut(Option<DateTime<Local>>) + 'a>,
}

impl<'a> DatePickerPopupComponent<'a> {
    pub fn new(
        selected_date: Option<DateTime<Local>>,
        observer: Box<dyn FnMut(Option<DateTime<Local>>) + 'a>,
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

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };

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
        match key.code {
            KeyCode::Tab => Some(Action::FocusNext),
            KeyCode::BackTab => Some(Action::FocusPrevious),
            KeyCode::Enter => Some(Action::Confirm),
            KeyCode::Char('h') => match self.focused_field {
                FocusField::Year | FocusField::Month | FocusField::Day => Some(Action::FocusLeft),
                FocusField::Calendar => Some(Action::MoveFocusedDate(-1)),
                FocusField::Cancel => None,
            },
            KeyCode::Char('l') => match self.focused_field {
                FocusField::Year | FocusField::Month | FocusField::Day => Some(Action::FocusRight),
                FocusField::Calendar => Some(Action::MoveFocusedDate(1)),
                FocusField::Cancel => None,
            },
            KeyCode::Char('j') => match self.focused_field {
                FocusField::Year | FocusField::Month | FocusField::Day => {
                    Some(Action::FocusCalendar)
                }
                FocusField::Calendar => Some(Action::MoveFocusedDate(7)),
                FocusField::Cancel => None,
            },
            KeyCode::Char('k') => match self.focused_field {
                FocusField::Year | FocusField::Month | FocusField::Day => {
                    Some(Action::FocusCalendar)
                }
                FocusField::Calendar => Some(Action::MoveFocusedDate(-7)),
                FocusField::Cancel => None,
            },
            KeyCode::Char('D') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                match self.focused_field {
                    FocusField::Calendar => Some(Action::MoveFocusedMonth(1)),
                    _ => None,
                }
            }
            KeyCode::Char('U') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                match self.focused_field {
                    FocusField::Calendar => Some(Action::MoveFocusedMonth(-1)),
                    _ => None,
                }
            }
            _ => match self.focused_field {
                FocusField::Year | FocusField::Month | FocusField::Day => {
                    Some(Action::InputKey(key))
                }
                FocusField::Calendar | FocusField::Cancel => None,
            },
        }
    }

    fn process_action(&mut self, action: Action) -> Option<EventProcessResult> {
        match action {
            Action::FocusNext => {
                self.focus_next();
                None
            }
            Action::FocusPrevious => {
                self.focus_previous();
                None
            }
            Action::FocusLeft => {
                self.focus_left();
                None
            }
            Action::FocusRight => {
                self.focus_right();
                None
            }
            Action::FocusCalendar => {
                self.focused_field = FocusField::Calendar;
                if let Some(input_date) = self.input_date() {
                    self.focused_date = input_date;
                    self.display_start = recent_sunday(input_date);
                }
                None
            }
            Action::MoveFocusedDate(days) => {
                self.move_focused_date(days);
                None
            }
            Action::MoveFocusedMonth(months) => {
                self.move_focused_month(months);
                None
            }
            Action::Confirm => self.confirm(),
            Action::InputKey(key) => {
                self.focused_textarea_mut().input(key);
                None
            }
        }
    }

    fn focus_next(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Year => FocusField::Month,
            FocusField::Month => FocusField::Day,
            FocusField::Day => FocusField::Calendar,
            FocusField::Calendar => FocusField::Cancel,
            FocusField::Cancel => FocusField::Cancel,
        };
    }

    fn focus_previous(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Year => FocusField::Year,
            FocusField::Month => FocusField::Year,
            FocusField::Day => FocusField::Month,
            FocusField::Calendar => FocusField::Day,
            FocusField::Cancel => FocusField::Calendar,
        };
    }

    fn focus_left(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Month => FocusField::Year,
            FocusField::Day => FocusField::Month,
            field => field,
        };
    }

    fn focus_right(&mut self) {
        self.focused_field = match self.focused_field {
            FocusField::Year => FocusField::Month,
            FocusField::Month => FocusField::Day,
            field => field,
        };
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
                let date = self.input_date()?;
                self.selected_date = Some(date);
                (self.observer)(Some(date));
                Some(EventProcessResult::Entered)
            }
            FocusField::Calendar => {
                self.selected_date = Some(self.focused_date);
                (self.observer)(Some(self.focused_date));
                Some(EventProcessResult::Entered)
            }
            FocusField::Cancel => {
                (self.observer)(None);
                Some(EventProcessResult::Canceled)
            }
        }
    }

    fn focused_textarea_mut(&mut self) -> &mut TextArea<'a> {
        match self.focused_field {
            FocusField::Year => &mut self.year_textarea,
            FocusField::Month => &mut self.month_textarea,
            FocusField::Day => &mut self.day_textarea,
            FocusField::Calendar | FocusField::Cancel => {
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
    use crate::test_support::local_datetime;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn key_event_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn component_with_observer(
        selected: Rc<RefCell<Option<Option<chrono::DateTime<chrono::Local>>>>>,
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
        let selected = Rc::new(RefCell::new(None));
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
    fn tab_and_shift_tab_move_focus_in_widget_order() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected);

        assert_eq!(component.focused_field, FocusField::Year);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Month);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Day);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Calendar);
        component.process_event(key_event(KeyCode::Tab));
        assert_eq!(component.focused_field, FocusField::Cancel);

        component.process_event(key_event(KeyCode::BackTab));
        assert_eq!(component.focused_field, FocusField::Calendar);
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
    fn j_and_k_move_from_each_date_input_to_calendar() {
        for field in [FocusField::Year, FocusField::Month, FocusField::Day] {
            for key in [KeyCode::Char('j'), KeyCode::Char('k')] {
                let selected = Rc::new(RefCell::new(None));
                let mut component = component_with_observer(selected);
                component.focused_field = field;

                component.process_event(key_event(key));

                assert_eq!(component.focused_field, FocusField::Calendar);
                assert_eq!(
                    component.focused_date,
                    local_datetime("2026-02-16T00:00:00+09:00")
                );
            }
        }
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
            KeyModifiers::SHIFT,
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
            KeyModifiers::SHIFT,
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
    fn enter_on_calendar_notifies_observer_with_focused_date() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.focused_field = FocusField::Calendar;
        component.process_event(key_event(KeyCode::Char('l')));

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Entered)));
        assert_eq!(
            *selected.borrow(),
            Some(Some(local_datetime("2026-02-17T00:00:00+09:00")))
        );
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
            Some(Some(local_datetime("2026-04-30T00:00:00+09:00")))
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

        assert!(result.is_none());
        assert_eq!(*selected.borrow(), None);
    }

    #[test]
    fn enter_on_cancel_notifies_observer_with_none() {
        let selected = Rc::new(RefCell::new(None));
        let mut component = component_with_observer(selected.clone());
        component.focused_field = FocusField::Cancel;

        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(matches!(result, Some(EventProcessResult::Canceled)));
        assert_eq!(*selected.borrow(), Some(None));
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
        let popup_area = DatePickerPopupWidget::popup_area(buffer.area);
        assert_eq!(
            buffer[(popup_area.x + 1, popup_area.y + popup_area.height - 4)].fg,
            ratatui::style::Color::LightGreen
        );
    }
}
