use chrono::{DateTime, Datelike, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use ratatui::widgets::calendar::{CalendarEventStore, Monthly};
use time::{Date, Month};

const FOCUSED_DATE_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const SELECTED_DATE_BG: Color = Color::Rgb(0x33, 0x26, 0x1A);

pub struct MonthlyCalendarWidget {
    display_start: DateTime<Local>,
    focused_date: Option<DateTime<Local>>,
    selected_date: Option<DateTime<Local>>,
}

impl MonthlyCalendarWidget {
    pub fn new(
        display_start: DateTime<Local>,
        focused_date: Option<DateTime<Local>>,
        selected_date: Option<DateTime<Local>>,
    ) -> Self {
        Self {
            display_start,
            focused_date,
            selected_date,
        }
    }
}

impl Widget for MonthlyCalendarWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Some(display_date) = chrono_to_time_date(self.display_start) else {
            return;
        };
        let mut events = CalendarEventStore::default();
        if let Some(selected_date) = self.selected_date.and_then(chrono_to_time_date) {
            events.add(
                selected_date,
                Style::default().fg(Color::White).bg(SELECTED_DATE_BG),
            );
        }
        if let Some(focused_date) = self.focused_date.and_then(chrono_to_time_date) {
            events.add(
                focused_date,
                Style::default().fg(Color::White).bg(FOCUSED_DATE_BG),
            );
        }

        Monthly::new(display_date, events)
            .show_month_header(Style::default().fg(Color::LightGreen))
            .show_weekdays_header(Style::default().fg(Color::DarkGray))
            .show_surrounding(Style::default().fg(Color::DarkGray))
            .render(area, buf);
    }
}

fn chrono_to_time_date(value: DateTime<Local>) -> Option<Date> {
    let date = value.date_naive();
    let month = Month::try_from(date.month() as u8).expect("chrono month is in 1..=12");

    Date::from_calendar_date(date.year(), month, date.day() as u8).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn snapshot_monthly_calendar_renders_selected_day() {
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-02-16T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-16T00:00:00+09:00");

        render_snapshot(
            "monthly_calendar_selected_day",
            25,
            8,
            MonthlyCalendarWidget::new(display_start, Some(focused_date), Some(selected_date)),
        );
    }

    #[test]
    fn snapshot_monthly_calendar_renders_at_minimum_useful_size() {
        let display_start = local_datetime("2026-04-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-04-30T00:00:00+09:00");
        let selected_date = local_datetime("2026-04-30T00:00:00+09:00");

        render_snapshot(
            "monthly_calendar_minimum_useful_size",
            21,
            7,
            MonthlyCalendarWidget::new(display_start, Some(focused_date), Some(selected_date)),
        );
    }

    #[test]
    fn render_does_not_focus_any_day_when_focused_date_is_outside_visible_range() {
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-04-16T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-20T00:00:00+09:00");
        let mut terminal = Terminal::new(TestBackend::new(25, 8)).unwrap();

        terminal
            .draw(|frame| {
                frame.render_widget(
                    MonthlyCalendarWidget::new(
                        display_start,
                        Some(focused_date),
                        Some(selected_date),
                    ),
                    frame.area(),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let focused_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == FOCUSED_DATE_BG)
            .count();
        let selected_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == SELECTED_DATE_BG)
            .count();
        assert_eq!(focused_cells, 0);
        assert_eq!(selected_cells, 2);
    }

    #[test]
    fn render_does_not_draw_popup_border_or_title() {
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-02-16T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-16T00:00:00+09:00");
        let mut terminal = Terminal::new(TestBackend::new(25, 8)).unwrap();

        terminal
            .draw(|frame| {
                frame.render_widget(
                    MonthlyCalendarWidget::new(
                        display_start,
                        Some(focused_date),
                        Some(selected_date),
                    ),
                    frame.area(),
                )
            })
            .unwrap();

        let rendered_text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!rendered_text.contains('┌'));
        assert!(!rendered_text.contains('┐'));
        assert!(!rendered_text.contains('└'));
        assert!(!rendered_text.contains('┘'));
        assert!(!rendered_text.contains("日付選択"));
    }

    #[test]
    fn render_uses_different_backgrounds_for_focused_and_selected_dates() {
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-02-16T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-20T00:00:00+09:00");
        let mut terminal = Terminal::new(TestBackend::new(25, 8)).unwrap();

        terminal
            .draw(|frame| {
                frame.render_widget(
                    MonthlyCalendarWidget::new(
                        display_start,
                        Some(focused_date),
                        Some(selected_date),
                    ),
                    frame.area(),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let focused_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == FOCUSED_DATE_BG)
            .count();
        let selected_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == SELECTED_DATE_BG)
            .count();

        assert_eq!(focused_cells, 2);
        assert_eq!(selected_cells, 2);
    }

    #[test]
    fn render_does_not_draw_focus_background_when_focused_date_is_none() {
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-20T00:00:00+09:00");
        let mut terminal = Terminal::new(TestBackend::new(25, 8)).unwrap();

        terminal
            .draw(|frame| {
                frame.render_widget(
                    MonthlyCalendarWidget::new(display_start, None, Some(selected_date)),
                    frame.area(),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let focused_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == FOCUSED_DATE_BG)
            .count();
        let selected_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == SELECTED_DATE_BG)
            .count();

        assert_eq!(focused_cells, 0);
        assert_eq!(selected_cells, 2);
    }

    #[test]
    fn render_does_not_draw_selected_background_when_selected_date_is_none() {
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-02-16T00:00:00+09:00");
        let mut terminal = Terminal::new(TestBackend::new(25, 8)).unwrap();

        terminal
            .draw(|frame| {
                frame.render_widget(
                    MonthlyCalendarWidget::new(display_start, Some(focused_date), None),
                    frame.area(),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let focused_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == FOCUSED_DATE_BG)
            .count();
        let selected_cells = buffer
            .content
            .iter()
            .filter(|cell| cell.bg == SELECTED_DATE_BG)
            .count();

        assert_eq!(focused_cells, 2);
        assert_eq!(selected_cells, 0);
    }
}
