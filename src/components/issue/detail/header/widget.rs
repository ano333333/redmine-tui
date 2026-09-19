use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::vos::IssueId;
use crate::widgets::theme::{ACCENT, BADGE_BG, FOCUS_BG, MUTED};

#[derive(Debug, Clone, Copy)]
pub enum TitleDecorater {
    Edited,
    Uploading,
}

pub struct HeaderWidget<'a> {
    id: IssueId,
    title: &'a str,
    focused_title: bool,
    title_decorator: Option<TitleDecorater>,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title_line_count = self.title_line_count(area.width);

        // ID・マーカー・タイトルを1行に詰め、縦の空行を捨てる(コンパクト型)
        if title_line_count > 0 {
            let title_area = Rect::new(area.x, area.y, area.width, title_line_count);
            self.title_paragraph().render(title_area, buf);
        }

        render_line(Line::from(""), area, buf, title_line_count);

        if self.focused_title {
            apply_background_to_rows(buf, area, 0, title_line_count);
        }
    }
}

impl<'a> HeaderWidget<'a> {
    pub fn new(
        id: impl Into<IssueId>,
        title: &'a str,
        focused_title: bool,
        title_decorator: Option<TitleDecorater>,
    ) -> Self {
        Self {
            id: id.into(),
            title,
            focused_title,
            title_decorator,
        }
    }

    pub fn line_count(&self, width: u16) -> usize {
        if width == 0 {
            return 0;
        }

        // タイトル行 + 下の空行1行のみ
        self.title_line_count(width) as usize + 1
    }

    fn title_line_count(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }

        self.title_paragraph().line_count(width) as u16
    }

    fn title_decorator_str(decorator: Option<TitleDecorater>) -> &'static str {
        match decorator {
            // FIXME: Nerd font対応
            Some(TitleDecorater::Edited) => "＊未保存",
            Some(TitleDecorater::Uploading) => "↑送信中",
            None => "",
        }
    }

    pub fn title_start_x(&self) -> u16 {
        // Paragraphのtrimによってタイトル直前の区切り空白が1セル詰められる。
        (Line::from(self.title_prefix_spans()).width() as u16).saturating_sub(1)
    }

    fn title_prefix_spans(&self) -> Vec<Span<'static>> {
        let mut spans = vec![
            Span::from(format!(" #{} ", self.id))
                .fg(ACCENT)
                .bg(BADGE_BG)
                .bold(),
            Span::from(" "),
        ];

        let decorator = Self::title_decorator_str(self.title_decorator);
        if !decorator.is_empty() {
            spans.push(Span::from(decorator).fg(MUTED));
            spans.push(Span::from(" "));
        }
        spans
    }

    fn title_paragraph(&self) -> Paragraph<'a> {
        // IDは反転背景のバッジにして、タイトルとの境目を色で示す
        let mut spans = self.title_prefix_spans();
        spans.push(Span::from(self.title).bold());

        Paragraph::new(Text::from(Line::from(spans))).wrap(Wrap { trim: true })
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
        let widget = HeaderWidget::new(42, &title, true, None);
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_wide_short_title", width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_narrow_long_title_wrap() {
        let title = "A very long title for observing current paragraph behavior".to_string();
        let width = 18;
        let widget = HeaderWidget::new(42, &title, false, None);
        let line_count = widget.line_count(width);
        // 折り返し4行 + 下の空行1行
        assert_eq!(line_count, 5);
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
        let widget = HeaderWidget::new(42, &title, false, None);
        assert_eq!(widget.line_count(40), 3);
        assert_eq!(widget.line_count(18), 5);
    }

    #[test]
    fn title_start_x_accounts_for_two_digit_issue_id() {
        let widget = HeaderWidget::new(42, "title", true, None);

        assert_eq!(widget.title_start_x(), 5);
    }

    #[test]
    fn snapshot_header_unsynced_title() {
        let title = "Widget snapshot baseline".to_string();
        let width = 40;
        let widget = HeaderWidget::new(42, &title, true, Some(TitleDecorater::Edited));
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_unsynced_title", width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_uploading_title() {
        let title = "Widget snapshot baseline".to_string();
        let width = 40;
        let widget = HeaderWidget::new(42, &title, true, Some(TitleDecorater::Uploading));
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_uploading_title", width, line_count as u16, widget);
    }
}
