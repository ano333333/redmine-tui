use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use super::theme::{ACCENT, BADGE_BG};

/// Storeやfocus状態に依存せず、渡された一時通知を右上へ重ねて描画するWidget。
pub struct ToastWidget {
    /// 表示順に並んだ通知メッセージ。
    pub messages: Vec<String>,
}

impl ToastWidget {
    /// 表示順に並んだ通知メッセージからWidgetを作る。
    pub fn new(messages: Vec<String>) -> Self {
        Self { messages }
    }

    /// 親領域の右上3分の1を通知の描画領域として返す。
    pub fn toast_area(area: Rect) -> Rect {
        let width = area.width / 3;
        Rect {
            x: area.x + area.width.saturating_sub(width + 1),
            y: area.y + 1,
            width,
            height: area.height.saturating_sub(2),
        }
    }

    fn required_height(&self, width: u16) -> u16 {
        let inner_width = width.saturating_sub(2);
        self.messages
            .iter()
            .map(|message| {
                let paragraph =
                    Paragraph::new(Text::from(message.as_str())).wrap(Wrap { trim: false });
                paragraph.line_count(inner_width) as u16 + 2
            })
            .sum()
    }
}

impl Widget for ToastWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.messages.is_empty() {
            return;
        }
        let area = Self::toast_area(area);
        // 枠と本文の最小領域を確保できない場合は、崩れた通知を出さない。
        if area.width < 4 || area.height < 3 {
            return;
        }

        let height = self.required_height(area.width).min(area.height);
        let area = Rect { height, ..area };

        Clear.render(area, buf);

        let mut y = area.y;
        for message in &self.messages {
            let remaining = area.y + area.height - y;
            if remaining < 2 {
                break;
            }
            let paragraph = Paragraph::new(Text::from(message.as_str())).wrap(Wrap { trim: false });
            let inner_width = area.width.saturating_sub(2);
            let height = (paragraph.line_count(inner_width) as u16 + 2).min(remaining);
            let toast_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height,
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(BADGE_BG))
                .border_style(Style::default().fg(ACCENT));
            let inner = block.inner(toast_area);
            block.render(toast_area, buf);
            paragraph.render(inner, buf);
            y += height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;
    use ratatui::buffer::Cell;

    #[test]
    fn snapshot_toast_single() {
        let widget = ToastWidget::new(vec![
            "Issueの保存に失敗しました: 接続が切れました".to_string(),
        ]);
        render_snapshot("toast_single", 60, 12, widget);
    }

    #[test]
    fn snapshot_toast_multiple() {
        let widget = ToastWidget::new(vec![
            "Issueの保存に失敗しました: 接続が切れました".to_string(),
            "エディタの起動に失敗しました".to_string(),
            "Journalの保存に失敗しました: タイムアウトしました".to_string(),
        ]);
        render_snapshot("toast_multiple", 60, 16, widget);
    }

    #[test]
    fn snapshot_toast_long_message_wraps() {
        let widget = ToastWidget::new(vec!["Issueの保存に失敗しました: Redmineサーバーへの接続が切れたため更新を送信できません。再接続してからもう一度保存してください。".to_string()]);
        render_snapshot("toast_long_message_wraps", 60, 12, widget);
    }

    #[test]
    fn snapshot_toast_narrow_area_renders_nothing() {
        let widget = ToastWidget::new(vec![
            "Issueの保存に失敗しました: 接続が切れました".to_string(),
        ]);
        render_snapshot("toast_narrow_area_renders_nothing", 10, 8, widget);
    }

    #[test]
    fn empty_toasts_render_nothing() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);

        ToastWidget::new(vec![]).render(area, &mut buf);

        assert!(buf.content.iter().all(|cell| *cell == Cell::EMPTY));
    }
}
