use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui_textarea::TextArea;

use crate::widgets::monthly_calendar_widget::MonthlyCalendarWidget;

const POPUP_WIDTH: u16 = 31;
const POPUP_HEIGHT_WITH_CALENDAR: u16 = 17;
const POPUP_HEIGHT_WITHOUT_CALENDAR: u16 = 8;

pub struct DatePickerPopupWidget<'a> {
    year_textarea: &'a TextArea<'a>,
    month_textarea: &'a TextArea<'a>,
    day_textarea: &'a TextArea<'a>,
    year_textarea_focused: bool,
    month_textarea_focused: bool,
    day_textarea_focused: bool,
    calendar_button_focused: bool,
    cancel_button_focused: bool,
    display_start: DateTime<Local>,
    /// Someの間だけカレンダーを表示する
    focused_date: Option<DateTime<Local>>,
    selected_date: Option<DateTime<Local>>,
}

impl<'a> DatePickerPopupWidget<'a> {
    pub fn new(
        year_textarea: &'a TextArea<'a>,
        month_textarea: &'a TextArea<'a>,
        day_textarea: &'a TextArea<'a>,
        year_textarea_focused: bool,
        month_textarea_focused: bool,
        day_textarea_focused: bool,
        calendar_button_focused: bool,
        cancel_button_focused: bool,
        display_start: DateTime<Local>,
        focused_date: Option<DateTime<Local>>,
        selected_date: Option<DateTime<Local>>,
    ) -> Self {
        Self {
            year_textarea,
            month_textarea,
            day_textarea,
            year_textarea_focused,
            month_textarea_focused,
            day_textarea_focused,
            calendar_button_focused,
            cancel_button_focused,
            display_start,
            focused_date,
            selected_date,
        }
    }

    pub fn popup_area(area: Rect, calendar_visible: bool) -> Rect {
        let popup_height = if calendar_visible {
            POPUP_HEIGHT_WITH_CALENDAR
        } else {
            POPUP_HEIGHT_WITHOUT_CALENDAR
        };
        let width = area.width.min(POPUP_WIDTH);
        let height = area.height.min(popup_height);

        Rect {
            x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
            y: area
                .y
                .saturating_add(area.height.saturating_sub(height) / 2),
            width,
            height,
        }
    }
}

impl Widget for DatePickerPopupWidget<'_> {
    /// クライアント領域に対する描画(したがってClearの責務がある)
    fn render(self, area: Rect, buf: &mut Buffer) {
        let calendar_visible = self.focused_date.is_some();
        let area = Self::popup_area(area, calendar_visible);
        if area.width < 3 || area.height < 3 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default().borders(Borders::ALL).title("日付選択");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let calendar_height = if calendar_visible { 1 } else { 0 };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(calendar_height),
                Constraint::Fill(calendar_height),
                Constraint::Length(3),
            ])
            .split(inner);
        let input_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(11),
                Constraint::Length(6),
                Constraint::Length(6),
            ])
            .split(rows[0]);

        render_textarea(
            self.year_textarea,
            input_cols[0],
            buf,
            "年",
            self.year_textarea_focused,
        );
        render_textarea(
            self.month_textarea,
            input_cols[1],
            buf,
            "月",
            self.month_textarea_focused,
        );
        render_textarea(
            self.day_textarea,
            input_cols[2],
            buf,
            "日",
            self.day_textarea_focused,
        );

        if calendar_visible {
            MonthlyCalendarWidget::new(self.display_start, self.focused_date, self.selected_date)
                .render(rows[2], buf);
        }
        render_buttons(
            rows[3],
            buf,
            !calendar_visible,
            self.calendar_button_focused,
            self.cancel_button_focused,
        );
    }
}

fn render_textarea(
    textarea: &TextArea<'_>,
    area: Rect,
    buf: &mut Buffer,
    title: &'static str,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let mut textarea = textarea.clone();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(textarea_border_style(focused)),
    );
    (&textarea).render(area, buf);
}

fn textarea_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn render_buttons(
    area: Rect,
    buf: &mut Buffer,
    calendar_button_visible: bool,
    calendar_button_focused: bool,
    cancel_button_focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Fill(1),
        ])
        .split(area);
    // カレンダー表示中もキャンセルボタンの位置は変えない
    if calendar_button_visible {
        render_button("カレンダー", cols[0], buf, calendar_button_focused);
    }
    render_button("キャンセル", cols[1], buf, cancel_button_focused);
}

fn render_button(label: &'static str, area: Rect, buf: &mut Buffer, focused: bool) {
    Paragraph::new(Line::from(label))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(button_border_style(focused)),
        )
        .render(area, buf);
}

fn button_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};
    use ratatui::{Terminal, backend::TestBackend};

    fn textarea_with_value(value: &str) -> TextArea<'static> {
        let mut textarea = TextArea::default();
        textarea.insert_str(value);
        textarea
    }

    fn render_popup_snapshot(
        name: &str,
        width: u16,
        height: u16,
        focused_date: Option<DateTime<Local>>,
    ) {
        let year_textarea = textarea_with_value("2026");
        let month_textarea = textarea_with_value("02");
        let day_textarea = textarea_with_value("16");
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-16T00:00:00+09:00");

        render_snapshot(
            name,
            width,
            height,
            DatePickerPopupWidget::new(
                &year_textarea,
                &month_textarea,
                &day_textarea,
                false,
                false,
                false,
                false,
                false,
                display_start,
                focused_date,
                Some(selected_date),
            ),
        );
    }

    #[test]
    fn snapshot_date_picker_popup_renders_whole_widget_for_client_sizes() {
        let focused_date = Some(local_datetime("2026-02-16T00:00:00+09:00"));
        render_popup_snapshot("date_picker_popup_whole_widget_60x24", 60, 24, focused_date);
        render_popup_snapshot("date_picker_popup_whole_widget_31x17", 31, 17, focused_date);
    }

    #[test]
    fn snapshot_date_picker_popup_hides_calendar_while_closed() {
        render_popup_snapshot("date_picker_popup_calendar_closed_60x24", 60, 24, None);
    }

    #[test]
    fn render_highlights_only_focused_textarea_border() {
        let year_textarea = textarea_with_value("2026");
        let month_textarea = textarea_with_value("02");
        let day_textarea = textarea_with_value("16");
        let display_start = local_datetime("2026-02-01T00:00:00+09:00");
        let focused_date = local_datetime("2026-02-16T00:00:00+09:00");
        let selected_date = local_datetime("2026-02-16T00:00:00+09:00");
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();

        terminal
            .draw(|frame| {
                frame.render_widget(
                    DatePickerPopupWidget::new(
                        &year_textarea,
                        &month_textarea,
                        &day_textarea,
                        false,
                        true,
                        false,
                        false,
                        false,
                        display_start,
                        Some(focused_date),
                        Some(selected_date),
                    ),
                    frame.area(),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let popup_area = DatePickerPopupWidget::popup_area(buffer.area, true);
        let input_y = popup_area.y + 1;
        let input_x = popup_area.x + 1;

        assert_eq!(buffer[(input_x, input_y)].fg, Color::DarkGray);
        assert_eq!(buffer[(input_x + 11, input_y)].fg, Color::LightGreen);
        assert_eq!(buffer[(input_x + 17, input_y)].fg, Color::DarkGray);
    }
}
