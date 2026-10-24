use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui_textarea::TextArea;

use super::component::Field;
use crate::widgets::theme::{ACCENT, MUTED, TIMELINE_FG};

const POPUP_WIDTH: u16 = 56;
/// 枠2行 + 入力欄3行×2 + 案内1行 + エラー1行。
const POPUP_HEIGHT: u16 = 10;

pub struct AttachmentInputPopupWidget<'a> {
    filename: &'a TextArea<'a>,
    path: &'a TextArea<'a>,
    focused: Field,
    is_incomplete: bool,
}

impl<'a> AttachmentInputPopupWidget<'a> {
    pub(super) fn new(
        filename: &'a TextArea<'a>,
        path: &'a TextArea<'a>,
        focused: Field,
        is_incomplete: bool,
    ) -> Self {
        Self {
            filename,
            path,
            focused,
            is_incomplete,
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

impl Widget for AttachmentInputPopupWidget<'_> {
    /// クライアント領域に対する描画(したがってClearの責務がある)
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Self::popup_area(area);
        if area.width < 3 || area.height < 3 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ファイルを添付 ")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(TIMELINE_FG));
        let inner = block.inner(area);
        block.render(area, buf);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        render_field(
            self.filename,
            "ファイル名",
            self.focused == Field::Filename,
            rows[0],
            buf,
        );
        render_field(
            self.path,
            "ファイルパス",
            self.focused == Field::Path,
            rows[1],
            buf,
        );

        Paragraph::new(Line::styled(
            "Enter: 確定 / Tab: 入力欄の切替 / Esc: 取消",
            Style::default().fg(MUTED),
        ))
        .render(rows[2], buf);

        if self.is_incomplete {
            Paragraph::new(Line::styled(
                "ファイル名とファイルパスを入力してください",
                Style::default().fg(Color::Red),
            ))
            .render(rows[3], buf);
        }
    }
}

fn render_field(textarea: &TextArea<'_>, title: &str, focused: bool, area: Rect, buf: &mut Buffer) {
    let mut textarea = textarea.clone();
    let border_color = if focused { ACCENT } else { TIMELINE_FG };
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(border_color)),
    );
    // 入力中でない欄にカーソルを表示しない。
    if !focused {
        textarea.set_cursor_style(Style::default());
    }
    (&textarea).render(area, buf);
}
