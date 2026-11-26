use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::vos::IssueId;
use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED, SECTION_BAR};
use crate::widgets::{VerticalScrollWidget, VerticalScrollWidgetState};

/// ラベル列の表示幅(全角換算)。値はこの位置から始まる。
const LABEL_WIDTH: usize = 14;
const HINT: &str = "Enter: 編集  Ctrl+S: 作成  y: Issue選択";
const SUBMITTING_HINT: &str = "作成中…  y: Issue選択";

/// 新規作成の下書きを、項目ごとの1行と添付ファイルの一覧で描画する。
///
/// 本文はフォーカス行が見えるように縦にスクロールし、最下行にキー操作のヒントを置く。
pub struct NewIssueWidget<'a> {
    pub scroll_state: &'a VerticalScrollWidgetState,
    pub project: &'a str,
    pub parent: Option<(IssueId, &'a str)>,
    /// 編集できる項目のラベルと表示値。添付ファイルの追加行の前に並ぶ。
    pub fields: Vec<(&'static str, String)>,
    pub attachments: Vec<String>,
    /// `fields`の後に添付ファイルの追加行を数えたindex。
    pub focused_index: usize,
    pub submitting: bool,
}

impl NewIssueWidget<'_> {
    /// 本文の先頭から数えた、フォーカス行の位置。
    pub fn focused_row(
        has_parent: bool,
        field_count: usize,
        attachment_count: usize,
        focused_index: usize,
    ) -> u16 {
        // 見出し、親Issue、空行の後に項目が並ぶ。
        let first_field_row = 2 + usize::from(has_parent);
        let row = if focused_index < field_count {
            first_field_row + focused_index
        } else {
            // 項目の後に空行と添付ファイルの見出し、添付ファイルが並び、最後が追加行。
            first_field_row + field_count + 2 + attachment_count
        };
        row as u16
    }

    /// 最下行のヒントを除いた、本文の表示行数。
    pub fn body_height(area_height: u16) -> u16 {
        area_height.saturating_sub(1)
    }

    fn body_lines(&self) -> Vec<Line<'_>> {
        let mut lines = vec![Line::from(vec![
            Span::from(SECTION_BAR).fg(ACCENT),
            Span::from("新しいIssue").bold(),
            Span::from(format!("  {}", self.project)).fg(MUTED),
        ])];
        if let Some((id, subject)) = self.parent {
            lines.push(Line::from(vec![
                Span::from(pad_label("親Issue")).fg(MUTED),
                Span::from(format!("#{id} {subject}")),
            ]));
        }
        lines.push(Line::default());
        for (label, value) in &self.fields {
            lines.push(Line::from(vec![
                Span::from(pad_label(label)).fg(MUTED),
                Span::from(value.as_str()),
            ]));
        }
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::from(SECTION_BAR).fg(ACCENT),
            Span::from("添付ファイル").bold(),
        ]));
        for filename in &self.attachments {
            lines.push(Line::from(format!("  {filename}")));
        }
        lines.push(Line::from("  + ファイルを追加").fg(ACCENT));
        lines
    }
}

impl Widget for NewIssueWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let body_area = Rect::new(area.x, area.y, area.width, Self::body_height(area.height));
        let lines = self.body_lines();
        let line_count = lines.len() as u16;
        let body = NewIssueBody {
            lines,
            focused_row: Self::focused_row(
                self.parent.is_some(),
                self.fields.len(),
                self.attachments.len(),
                self.focused_index,
            ),
        };
        let mut scroll_widget = VerticalScrollWidget::new(
            self.scroll_state,
            Size::new(body_area.width, body_area.height),
        );
        scroll_widget.render_widget(body, line_count);
        scroll_widget.render(body_area, buf);
        if area.height > 0 {
            let hint = if self.submitting {
                SUBMITTING_HINT
            } else {
                HINT
            };
            Paragraph::new(Line::from(hint).fg(MUTED)).render(
                Rect::new(area.x, area.y + area.height - 1, area.width, 1),
                buf,
            );
        }
    }
}

/// スクロールしない本文全体。フォーカス行の背景を塗る。
struct NewIssueBody<'a> {
    lines: Vec<Line<'a>>,
    focused_row: u16,
}

impl Widget for NewIssueBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines).render(area, buf);
        if self.focused_row < area.height {
            buf.set_style(
                Rect::new(area.x, area.y + self.focused_row, area.width, 1),
                Style::default().bg(FOCUS_BG),
            );
        }
    }
}

/// ラベルを表示幅 LABEL_WIDTH まで空白で埋める。全角文字は2桁として数える。
fn pad_label(label: &str) -> String {
    let display_width: usize = label
        .chars()
        .map(|c| if c.is_ascii() { 1 } else { 2 })
        .sum();
    format!(
        "{}{}",
        label,
        " ".repeat(LABEL_WIDTH.saturating_sub(display_width))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    fn fields() -> Vec<(&'static str, String)> {
        vec![
            ("題名", "ログイン画面の不具合".to_string()),
            ("説明", "-".to_string()),
            ("トラッカー", "(既定)".to_string()),
        ]
    }

    #[test]
    fn snapshot_child_issue_draft_with_attachments() {
        render_snapshot(
            "new_issue_child_draft_with_attachments",
            60,
            14,
            NewIssueWidget {
                scroll_state: &VerticalScrollWidgetState::new(),
                project: "Sample Project",
                parent: Some((IssueId::new(3), "親のIssue")),
                fields: fields(),
                attachments: vec!["screen.png".to_string(), "log.txt".to_string()],
                focused_index: 1,
                submitting: false,
            },
        );
    }

    #[test]
    fn snapshot_issue_draft_focusing_the_add_attachment_row() {
        render_snapshot(
            "new_issue_draft_focusing_add_attachment",
            60,
            12,
            NewIssueWidget {
                scroll_state: &VerticalScrollWidgetState::new(),
                project: "Sample Project",
                parent: None,
                fields: fields(),
                attachments: Vec::new(),
                focused_index: 3,
                submitting: false,
            },
        );
    }

    #[test]
    fn snapshot_submitting_issue_draft() {
        render_snapshot(
            "new_issue_submitting_draft",
            60,
            12,
            NewIssueWidget {
                scroll_state: &VerticalScrollWidgetState::new(),
                project: "Sample Project",
                parent: None,
                fields: fields(),
                attachments: Vec::new(),
                focused_index: 0,
                submitting: true,
            },
        );
    }
}
