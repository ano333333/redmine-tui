use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget};

use crate::stores::AttachmentUploadStage;
use crate::widgets::gutter::{Gutter, indented_area};
use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED, SECTION_BAR};

/// 見出しとその下の空行。縦線はこの下の一覧部分にだけ引く。
pub const HEADER_LINES: u16 = 2;
/// 縦線を引かない末尾の余白行。次のブロックとの区切りになる。
const GUTTER_TRAILING_LINES: u16 = 1;

pub struct AttachmentRow<'a> {
    pub filename: &'a str,
    pub filesize: u64,
    pub author: &'a str,
    pub created_on: DateTime<Local>,
}

pub struct UploadingRow<'a> {
    pub filename: &'a str,
    pub stage: AttachmentUploadStage,
}

/// 添付ファイルの行の後に、追加中のファイルがあればその行を、最後に追加行を描画する。
pub struct AttachmentsListWidget<'a> {
    attachments: Vec<AttachmentRow<'a>>,
    uploading: Option<UploadingRow<'a>>,
    /// Issue属性・Journal・添付ファイルの保存中は追加できないため、追加行を無効の色で描画する。
    add_enabled: bool,
    focused_index: Option<usize>,
}

impl<'a> AttachmentsListWidget<'a> {
    pub fn new(
        attachments: Vec<AttachmentRow<'a>>,
        uploading: Option<UploadingRow<'a>>,
        add_enabled: bool,
        focused_index: Option<usize>,
    ) -> Self {
        Self {
            attachments,
            uploading,
            add_enabled,
            focused_index,
        }
    }

    pub fn line_count(&self) -> u16 {
        HEADER_LINES + self.row_count() as u16 + GUTTER_TRAILING_LINES
    }

    fn row_count(&self) -> usize {
        row_count(self.attachments.len(), self.uploading.is_some())
    }
}

/// 添付ファイル + 追加中の行 + 追加行。
pub fn row_count(attachment_count: usize, uploading: bool) -> usize {
    attachment_count + usize::from(uploading) + 1
}

impl Widget for AttachmentsListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        create_header_text(self.attachments.len()).render(area, buf);

        let list_area = Rect::new(
            area.x,
            area.y + HEADER_LINES,
            area.width,
            area.height.saturating_sub(HEADER_LINES),
        );
        let gutter_area = Rect::new(
            list_area.x,
            list_area.y,
            list_area.width,
            list_area.height.min(
                self.line_count()
                    .saturating_sub(HEADER_LINES + GUTTER_TRAILING_LINES),
            ),
        );
        Gutter::line().render(gutter_area, buf);

        let mut row_area = indented_area(list_area);
        let mut index = 0;
        let mut next_row = |row_area: &mut Rect, buf: &mut Buffer| {
            highlight_if_focused(*row_area, buf, self.focused_index == Some(index));
            index += 1;
            row_area.y += 1;
            row_area.height = row_area.height.saturating_sub(1);
        };
        for attachment in &self.attachments {
            render_attachment(attachment, row_area, buf);
            next_row(&mut row_area, buf);
        }
        if let Some(uploading) = &self.uploading {
            render_uploading(uploading, row_area, buf);
            next_row(&mut row_area, buf);
        }
        if row_area.height > 0 {
            let color = if self.add_enabled { ACCENT } else { MUTED };
            Paragraph::new(Line::from("＋ ファイルを追加").fg(color)).render(row_area, buf);
            next_row(&mut row_area, buf);
        }
    }
}

fn create_header_text(attachment_count: usize) -> Text<'static> {
    Text::from(vec![
        Line::from(vec![
            Span::from(SECTION_BAR).fg(ACCENT),
            Span::from(" 添付ファイル ").bold(),
            Span::from(attachment_count.to_string()).fg(ACCENT).bold(),
        ]),
        Line::from(""),
    ])
}

fn render_attachment(attachment: &AttachmentRow, area: Rect, buf: &mut Buffer) {
    let row = Rect::new(area.x, area.y, area.width, area.height.min(1));
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1), // filename
            Constraint::Length(1),
            Constraint::Length(9), // filesize
            Constraint::Length(1),
            Constraint::Length(14), // author
            Constraint::Length(1),
            Constraint::Length(10), // created_on
        ])
        .split(row);
    Paragraph::new(attachment.filename.to_string()).render(cols[0], buf);
    Paragraph::new(format_filesize(attachment.filesize))
        .right_aligned()
        .fg(MUTED)
        .render(cols[2], buf);
    Paragraph::new(attachment.author.to_string()).render(cols[4], buf);
    Paragraph::new(attachment.created_on.format("%Y/%m/%d").to_string())
        .fg(MUTED)
        .render(cols[6], buf);
}

fn render_uploading(uploading: &UploadingRow, area: Rect, buf: &mut Buffer) {
    let stage = match uploading.stage {
        AttachmentUploadStage::Uploading => "送信中",
        AttachmentUploadStage::Attaching => "添付中",
        AttachmentUploadStage::Confirming => "確認中",
    };
    Paragraph::new(Line::from(vec![
        Span::from(uploading.filename.to_string()),
        Span::from(format!(" ({stage})")).yellow(),
    ]))
    .render(
        Rect::new(area.x, area.y, area.width, area.height.min(1)),
        buf,
    );
}

fn highlight_if_focused(area: Rect, buf: &mut Buffer, focused: bool) {
    if !focused || area.height == 0 {
        return;
    }
    for x in 0..area.width {
        if let Some(cell) = buf.cell_mut((area.x + x, area.y)) {
            cell.set_bg(FOCUS_BG);
        }
    }
}

/// 1024を単位に、B・KB・MB・GBのうち値が1024未満になる単位で表す。KB以上は小数1桁にする。
fn format_filesize(filesize: u64) -> String {
    const UNITS: [&str; 3] = ["KB", "MB", "GB"];
    if filesize < 1024 {
        return format!("{filesize} B");
    }
    let mut value = filesize as f64 / 1024.0;
    let mut unit = UNITS[0];
    for next_unit in &UNITS[1..] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next_unit;
    }
    format!("{value:.1} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};

    #[test]
    fn filesize_uses_the_largest_unit_below_1024() {
        assert_eq!(format_filesize(1023), "1023 B");
        assert_eq!(format_filesize(1024), "1.0 KB");
        assert_eq!(format_filesize(428_397), "418.4 KB");
        assert_eq!(format_filesize(1_471_702), "1.4 MB");
    }

    #[test]
    fn snapshot_attachments_with_focused_add_row() {
        let widget = AttachmentsListWidget::new(
            vec![AttachmentRow {
                filename: "sample.png",
                filesize: 1_471_702,
                author: "user1 Fixture",
                created_on: local_datetime("2026-02-05T09:00:00+09:00"),
            }],
            None,
            true,
            Some(1),
        );
        let line_count = widget.line_count();

        render_snapshot("attachments_list_focused_add_row", 64, line_count, widget);
    }

    #[test]
    fn snapshot_empty_attachments_list() {
        let widget = AttachmentsListWidget::new(vec![], None, true, None);
        let line_count = widget.line_count();

        render_snapshot("attachments_list_empty", 64, line_count, widget);
    }

    #[test]
    fn snapshot_uploading_row_and_disabled_add_row() {
        let widget = AttachmentsListWidget::new(
            vec![],
            Some(UploadingRow {
                filename: "screen.png",
                stage: AttachmentUploadStage::Attaching,
            }),
            false,
            None,
        );
        let line_count = widget.line_count();

        render_snapshot(
            "attachments_list_uploading_and_disabled_add_row",
            64,
            line_count,
            widget,
        );
    }
}
