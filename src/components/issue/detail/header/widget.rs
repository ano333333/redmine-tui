use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use super::focus_state::FocusedRow;
use crate::vos::IssueId;
use crate::widgets::theme::{ACCENT, BADGE_BG, FOCUS_BG, MUTED};

const PARENT_LABEL: &str = "親チケット ";

#[derive(Debug, Clone, Copy)]
pub enum TitleDecorater {
    Edited,
    Uploading,
    /// PUTは成功したが、確認の取得に失敗した。
    Unconfirmed,
}

/// 題名の下に出す親Issue。親の題名は、親Issueの詳細を取得できたときだけ分かる。
#[derive(Debug, Clone, Copy)]
pub enum ParentIssue<'a> {
    Loaded { id: IssueId, subject: &'a str },
    Fetching { id: IssueId },
    FetchFailed { id: IssueId },
}

pub struct HeaderWidget<'a> {
    id: IssueId,
    title: &'a str,
    parent: Option<ParentIssue<'a>>,
    focused_row: Option<FocusedRow>,
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

        let parent_line_count = self.parent_line_count(area.width);
        if let Some(parent_paragraph) = self.parent_paragraph()
            && title_line_count < area.height
        {
            let parent_area = Rect::new(
                area.x,
                area.y + title_line_count,
                area.width,
                parent_line_count.min(area.height - title_line_count),
            );
            parent_paragraph.render(parent_area, buf);
        }

        render_line(
            Line::from(""),
            area,
            buf,
            title_line_count + parent_line_count,
        );

        match self.focused_row {
            Some(FocusedRow::Title) => apply_background_to_rows(buf, area, 0, title_line_count),
            Some(FocusedRow::Parent) => {
                apply_background_to_rows(buf, area, title_line_count, parent_line_count)
            }
            None => {}
        }
    }
}

impl<'a> HeaderWidget<'a> {
    pub fn new(
        id: impl Into<IssueId>,
        title: &'a str,
        parent: Option<ParentIssue<'a>>,
        focused_row: Option<FocusedRow>,
        title_decorator: Option<TitleDecorater>,
    ) -> Self {
        Self {
            id: id.into(),
            title,
            parent,
            focused_row,
            title_decorator,
        }
    }

    pub fn line_count(&self, width: u16) -> usize {
        if width == 0 {
            return 0;
        }

        // タイトル行 + 親Issueの行 + 下の空行1行
        (self.title_line_count(width) + self.parent_line_count(width)) as usize + 1
    }

    /// 親Issueの行で、親のIDが始まる位置。親Issueがなければ`None`。
    pub fn parent_id_position(&self, width: u16) -> Option<Position> {
        self.parent?;
        Some(Position::new(
            Line::from(PARENT_LABEL).width() as u16,
            self.title_line_count(width),
        ))
    }

    fn parent_line_count(&self, width: u16) -> u16 {
        self.parent_paragraph()
            .map_or(0, |paragraph| paragraph.line_count(width) as u16)
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
            Some(TitleDecorater::Unconfirmed) => "？未確認",
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

    fn parent_paragraph(&self) -> Option<Paragraph<'a>> {
        let parent = self.parent?;
        let (id, subject) = match parent {
            ParentIssue::Loaded { id, subject } => (id, Span::from(subject)),
            ParentIssue::Fetching { id } => (id, Span::from("取得中…").fg(MUTED)),
            ParentIssue::FetchFailed { id } => (id, Span::from("取得失敗").fg(MUTED)),
        };
        let spans = vec![
            Span::from(PARENT_LABEL).fg(MUTED),
            Span::from(format!("#{id}")).blue(),
            Span::from(" "),
            subject,
        ];
        Some(Paragraph::new(Text::from(Line::from(spans))).wrap(Wrap { trim: true }))
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
        let widget = HeaderWidget::new(42, &title, None, Some(FocusedRow::Title), None);
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_wide_short_title", width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_narrow_long_title_wrap() {
        let title = "A very long title for observing current paragraph behavior".to_string();
        let width = 18;
        let widget = HeaderWidget::new(42, &title, None, None, None);
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
        let widget = HeaderWidget::new(42, &title, None, None, None);
        assert_eq!(widget.line_count(40), 3);
        assert_eq!(widget.line_count(18), 5);
    }

    #[test]
    fn title_start_x_accounts_for_two_digit_issue_id() {
        let widget = HeaderWidget::new(42, "title", None, Some(FocusedRow::Title), None);

        assert_eq!(widget.title_start_x(), 5);
    }

    #[test]
    fn snapshot_header_unsynced_title() {
        let title = "Widget snapshot baseline".to_string();
        let width = 40;
        let widget = HeaderWidget::new(
            42,
            &title,
            None,
            Some(FocusedRow::Title),
            Some(TitleDecorater::Edited),
        );
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_unsynced_title", width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_uploading_title() {
        let title = "Widget snapshot baseline".to_string();
        let width = 40;
        let widget = HeaderWidget::new(
            42,
            &title,
            None,
            Some(FocusedRow::Title),
            Some(TitleDecorater::Uploading),
        );
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_uploading_title", width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_unconfirmed_title() {
        let title = "Widget snapshot baseline".to_string();
        let width = 40;
        let widget = HeaderWidget::new(
            42,
            &title,
            None,
            Some(FocusedRow::Title),
            Some(TitleDecorater::Unconfirmed),
        );
        let line_count = widget.line_count(width);
        // タイトル1行 + 下の空行1行
        assert_eq!(line_count, 2);
        render_snapshot("header_unconfirmed_title", width, line_count as u16, widget);
    }

    fn render_header_with_parent(name: &str, parent: ParentIssue<'_>) {
        let width = 40;
        let widget = HeaderWidget::new(
            42,
            "Child issue",
            Some(parent),
            Some(FocusedRow::Parent),
            None,
        );
        let line_count = widget.line_count(width);
        // タイトル1行 + 親Issue1行 + 下の空行1行
        assert_eq!(line_count, 3);
        render_snapshot(name, width, line_count as u16, widget);
    }

    #[test]
    fn snapshot_header_loaded_parent() {
        render_header_with_parent(
            "header_loaded_parent",
            ParentIssue::Loaded {
                id: 3.into(),
                subject: "Parent issue",
            },
        );
    }

    #[test]
    fn snapshot_header_fetching_parent() {
        render_header_with_parent(
            "header_fetching_parent",
            ParentIssue::Fetching { id: 3.into() },
        );
    }

    #[test]
    fn snapshot_header_fetch_failed_parent() {
        render_header_with_parent(
            "header_fetch_failed_parent",
            ParentIssue::FetchFailed { id: 3.into() },
        );
    }
}
