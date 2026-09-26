use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui_textarea::TextArea;

use crate::widgets::theme::{ACCENT, MUTED, TIMELINE_FG};

const POPUP_WIDTH: u16 = 40;
/// 枠2行 + 入力欄3行 + 案内1行 + エラー1行。
const POPUP_HEIGHT: u16 = 7;

pub struct NumberInputPopupWidget<'a> {
    title: &'a str,
    textarea: &'a TextArea<'a>,
    is_invalid: bool,
}

impl<'a> NumberInputPopupWidget<'a> {
    pub fn new(title: &'a str, textarea: &'a TextArea<'a>, is_invalid: bool) -> Self {
        Self {
            title,
            textarea,
            is_invalid,
        }
    }

    fn popup_area(area: Rect) -> Rect {
        let width = POPUP_WIDTH.min(area.width);
        let height = POPUP_HEIGHT.min(area.height);
        Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        }
    }
}

impl Widget for NumberInputPopupWidget<'_> {
    /// クライアント領域に対する描画(したがってClearの責務がある)
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Self::popup_area(area);
        if area.width < 3 || area.height < 3 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title))
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(TIMELINE_FG));
        let inner = block.inner(area);
        block.render(area, buf);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let mut textarea = self.textarea.clone();
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        );
        (&textarea).render(rows[0], buf);

        Paragraph::new(Line::styled(
            "Enter: 確定 / Esc: 取消 / 空欄: 未設定",
            Style::default().fg(MUTED),
        ))
        .render(rows[1], buf);

        if self.is_invalid {
            Paragraph::new(Line::styled(
                "数値が大きすぎます",
                Style::default().fg(Color::Red),
            ))
            .render(rows[2], buf);
        }
    }
}
