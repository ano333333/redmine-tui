use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::vos::IssueId;
use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED, SECTION_BAR};

/// ラベル列の表示幅(全角換算)。値はこの位置から始まる。
const LABEL_WIDTH: usize = 14;
const HINT: &str = "Enter: 編集  Ctrl+S: 作成  y: Issue選択";

/// 新規作成の下書きを、項目ごとの1行と添付ファイルの一覧で描画する。
pub struct NewIssueWidget<'a> {
    pub project: &'a str,
    pub parent: Option<(IssueId, &'a str)>,
    /// 編集できる項目のラベルと表示値。添付ファイルの追加行の前に並ぶ。
    pub fields: Vec<(&'static str, String)>,
    pub attachments: Vec<&'a str>,
    /// `fields`の後に添付ファイルの追加行を数えたindex。
    pub focused_index: usize,
}

impl Widget for NewIssueWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
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
        let first_field_row = lines.len();
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
        let add_row = lines.len();
        lines.push(Line::from("  + ファイルを追加").fg(ACCENT));

        let focused_row = if self.focused_index < self.fields.len() {
            first_field_row + self.focused_index
        } else {
            add_row
        };
        let body = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
        Paragraph::new(lines).render(body, buf);
        if (focused_row as u16) < body.height {
            buf.set_style(
                Rect::new(area.x, area.y + focused_row as u16, area.width, 1),
                Style::default().bg(FOCUS_BG),
            );
        }
        if area.height > 0 {
            Paragraph::new(Line::from(HINT).fg(MUTED)).render(
                Rect::new(area.x, area.y + area.height - 1, area.width, 1),
                buf,
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
                project: "Sample Project",
                parent: Some((IssueId::new(3), "親のIssue")),
                fields: fields(),
                attachments: vec!["screen.png", "log.txt"],
                focused_index: 1,
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
                project: "Sample Project",
                parent: None,
                fields: fields(),
                attachments: Vec::new(),
                focused_index: 3,
            },
        );
    }
}
