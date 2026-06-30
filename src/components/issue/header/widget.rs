use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct HeaderWidget<'a> {
    id: u16,
    title: &'a str,
    focused_title: bool,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title_line_count = self.title_line_count(area.width);

        render_line(Line::from(format!("#{}", self.id)), area, buf, 0);
        render_line(Line::from(""), area, buf, 1);

        if title_line_count > 0 {
            let title_area = Rect::new(area.x, area.y + 2, area.width, title_line_count);
            self.title_paragraph().render(title_area, buf);
        }

        render_line(Line::from(""), area, buf, 2 + title_line_count);

        if self.focused_title {
            apply_background_to_rows(buf, area, 2, title_line_count);
        }
    }
}

impl<'a> HeaderWidget<'a> {
    pub fn new(id: u16, title: &'a str, focused_title: bool) -> Self {
        Self {
            id,
            title,
            focused_title,
        }
    }

    pub fn line_count(&self, width: u16) -> usize {
        if width == 0 {
            return 0;
        }

        self.title_line_count(width) as usize + 3
    }

    fn title_line_count(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }

        self.title_paragraph().line_count(width) as u16
    }

    fn title_paragraph(&self) -> Paragraph<'a> {
        Paragraph::new(Text::from(
            Line::from(format!("# {}", self.title)).style(Style::default().bold()),
        ))
        .wrap(Wrap { trim: true })
    }
}

fn render_line(line: Line<'_>, area: Rect, buf: &mut Buffer, row: u16) {
    if row >= area.height {
        return;
    }

    Paragraph::new(Text::from(line)).render(Rect::new(area.x, area.y + row, area.width, 1), buf);
}

fn apply_background_to_rows(buf: &mut Buffer, area: Rect, start_row: u16, row_count: u16) {
    for row in start_row..start_row.saturating_add(row_count) {
        if row >= area.height {
            return;
        }

        for x in 0..area.width {
            if let Some(cell) = buf.cell_mut((area.x + x, area.y + row)) {
                cell.set_bg(FOCUS_BG);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_header_wide_short_title() {
        let title = "Widget snapshot baseline".to_string();
        let width = 40;
        let widget = HeaderWidget::new(42, &title, true);
        let line_count = widget.line_count(width);
        assert_eq!(line_count, 4);
        render_snapshot("header_wide_short_title", width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_narrow_long_title_wrap() {
        let title = "A very long title for observing current paragraph behavior".to_string();
        let width = 18;
        let widget = HeaderWidget::new(42, &title, false);
        let line_count = widget.line_count(width);
        assert_eq!(line_count, 7);
        render_snapshot(
            "header_narrow_long_title_wrap",
            width,
            line_count as u16,
            widget,
        );
    }

    #[test]
    fn line_count_header_grows_when_title_wraps() {
        let title = "A very long title for observing current paragraph behavior".to_string();
        let widget = HeaderWidget::new(42, &title, false);
        assert_eq!(widget.line_count(40), 5);
        assert_eq!(widget.line_count(18), 7);
    }
}
