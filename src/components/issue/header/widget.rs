use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct HeaderWidget<'a> {
    id: u16,
    title: &'a String,
    creator: &'a String,
    appended_at: DateTime<Local>,
    updated_at: DateTime<Local>,
    focused_title: bool,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let focused_title = self.focused_title;
        let paragraph = Paragraph::new(vec![
            Line::from(format!("#{}", self.id)),
            Line::from(""),
            Line::from(format!("# {}", self.title.clone())).style(Style::default().bold()),
            // FIXME: 改行を指定して2行の間を作ろうとしているが、実際は1行分の空白しかできていない
            Line::from("\n"),
            Line::from(vec![
                Span::from(self.creator.clone()).style(Style::default().blue()),
                Span::from("が"),
                Span::from(self.appended_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に追加. "),
                Span::from(self.updated_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に更新."),
            ]),
            // FIXME: 改行2つで3行の間を作ろうとしているが、実際は1行分の空白しかできていない
            Line::from("\n\n"),
        ]);
        paragraph.render(area, buf);
        if focused_title {
            apply_background_to_row(buf, area, 2);
        }
    }
}

impl<'a> HeaderWidget<'a> {
    pub fn new(
        id: u16,
        title: &'a String,
        creator: &'a String,
        appended_at: DateTime<Local>,
        updated_at: DateTime<Local>,
        focused_title: bool,
    ) -> Self {
        Self {
            id,
            title,
            creator,
            appended_at,
            updated_at,
            focused_title,
        }
    }

    pub fn line_count(&self, _: u16) -> usize {
        6
    }
}

fn apply_background_to_row(buf: &mut Buffer, area: Rect, row: u16) {
    if row >= area.height {
        return;
    }

    for x in 0..area.width {
        if let Some(cell) = buf.cell_mut((area.x + x, area.y + row)) {
            cell.set_bg(FOCUS_BG);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};

    #[test]
    fn snapshot_header_wide_short_title() {
        let title = "Widget snapshot baseline".to_string();
        let creator = "alice".to_string();
        render_snapshot(
            "header_wide_short_title",
            40,
            6,
            HeaderWidget::new(
                42,
                &title,
                &creator,
                local_datetime("2026-01-10T00:00:00+09:00"),
                local_datetime("2026-01-15T00:00:00+09:00"),
                true,
            ),
        );
    }

    #[test]
    fn snapshot_header_narrow_long_title_no_wrap() {
        let title = "A very long title for observing current paragraph behavior".to_string();
        let creator = "alice".to_string();
        render_snapshot(
            "header_narrow_long_title_no_wrap",
            18,
            6,
            HeaderWidget::new(
                42,
                &title,
                &creator,
                local_datetime("2026-01-10T00:00:00+09:00"),
                local_datetime("2026-01-15T00:00:00+09:00"),
                false,
            ),
        );
    }

    #[test]
    fn line_count_header_is_fixed_6() {
        let title = "A very long title for observing current paragraph behavior".to_string();
        let creator = "alice".to_string();
        let widget = HeaderWidget::new(
            42,
            &title,
            &creator,
            local_datetime("2026-01-10T00:00:00+09:00"),
            local_datetime("2026-01-15T00:00:00+09:00"),
            false,
        );
        assert_eq!(widget.line_count(18), 6);
    }
}
