use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap};

use crate::widgets::VerticalScrollWidget;
use crate::widgets::theme::{ACCENT, FOCUS_BG};

const MESSAGE: &str = "Journal本文が競合しています。採用する方を選択してください。";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteJournalConflictChoice {
    Local,
    Server,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteJournalConflictButton {
    Cancel,
    Continue,
}

pub struct RemoteJournalConflictPopupWidget<'a> {
    content: VerticalScrollWidget<'a>,
}

impl<'a> RemoteJournalConflictPopupWidget<'a> {
    pub fn new(content: VerticalScrollWidget<'a>) -> Self {
        Self { content }
    }

    pub fn popup_area(area: Rect) -> Rect {
        let width = area.width.saturating_mul(9) / 10;
        let height = area.height.saturating_mul(4) / 5;
        Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        }
    }

    pub fn content_area(area: Rect) -> Rect {
        Block::default()
            .borders(Borders::ALL)
            .inner(Self::popup_area(area))
    }
}

impl Widget for RemoteJournalConflictPopupWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = Self::popup_area(area);
        if popup_area.width < 3 || popup_area.height < 3 {
            return;
        }
        Clear.render(popup_area, buf);
        let block = Block::default().borders(Borders::ALL);
        let content_area = block.inner(popup_area);
        block.render(popup_area, buf);
        self.content.render(content_area, buf);
    }
}

/// ローカルとサーバーの本文を左右で比較するWidget。
pub struct RemoteJournalConflictWidget {
    local: String,
    server: String,
    selected_choice: RemoteJournalConflictChoice,
    focused_button: Option<RemoteJournalConflictButton>,
}

impl RemoteJournalConflictWidget {
    pub fn new(
        local: impl Into<String>,
        server: impl Into<String>,
        selected_choice: RemoteJournalConflictChoice,
    ) -> Self {
        Self {
            local: local.into(),
            server: server.into(),
            selected_choice,
            focused_button: None,
        }
    }

    pub fn with_focused_button(
        mut self,
        focused_button: Option<RemoteJournalConflictButton>,
    ) -> Self {
        self.focused_button = focused_button;
        self
    }

    pub fn line_count(&self, width: u16) -> usize {
        if width == 0 {
            return 0;
        }
        let [local, server] = choice_areas(Rect::new(0, 0, width, 1));
        let choices = markdown_line_count(&self.local, local.width)
            .max(markdown_line_count(&self.server, server.width));
        Paragraph::new(MESSAGE)
            .line_count(width)
            .saturating_add(1)
            .saturating_add(1)
            .saturating_add(choices)
            .saturating_add(1)
            .saturating_add(3)
    }

    pub fn cursor_position_for_choice(
        &self,
        choice: RemoteJournalConflictChoice,
        width: u16,
    ) -> Position {
        let [local, server] = choice_areas(Rect::new(0, 0, width, 1));
        Position {
            x: match choice {
                RemoteJournalConflictChoice::Local => local.x,
                RemoteJournalConflictChoice::Server => server.x,
            },
            y: Paragraph::new(MESSAGE)
                .line_count(width)
                .saturating_add(1)
                .saturating_add(1) as u16,
        }
    }

    pub fn cursor_position_for_button(
        &self,
        button: RemoteJournalConflictButton,
        width: u16,
    ) -> Position {
        Position {
            x: button_x(button, width),
            y: self.line_count(width).saturating_sub(2) as u16,
        }
    }
}

impl Widget for RemoteJournalConflictWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let message_lines = Paragraph::new(MESSAGE).line_count(area.width) as u16;
        let [local, server] = choice_areas(Rect::new(0, 0, area.width, 1));
        let choice_lines = markdown_line_count(&self.local, local.width)
            .max(markdown_line_count(&self.server, server.width)) as u16;
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(message_lines),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(choice_lines),
                Constraint::Length(1),
                Constraint::Length(3),
            ])
            .split(area);

        Paragraph::new(MESSAGE)
            .wrap(Wrap { trim: false })
            .render(rows[0], buf);
        let [local_label, server_label] = choice_areas(rows[2]);
        render_label("LOCAL · あなたの変更", local_label, buf, Color::LightGreen);
        render_label(
            "REMOTE · サーバーの変更",
            server_label,
            buf,
            Color::LightBlue,
        );
        let [local_area, server_area] = choice_areas(rows[3]);
        render_choice(
            &self.local,
            local_area,
            buf,
            Color::LightGreen,
            self.selected_choice == RemoteJournalConflictChoice::Local,
        );
        render_choice(
            &self.server,
            server_area,
            buf,
            Color::LightBlue,
            self.selected_choice == RemoteJournalConflictChoice::Server,
        );
        render_buttons(rows[5], buf, self.focused_button);
    }
}

fn choice_areas(area: Rect) -> [Rect; 2] {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(area);
    [cols[0], cols[2]]
}

fn render_label(text: &'static str, area: Rect, buf: &mut Buffer, color: Color) {
    Paragraph::new(Line::styled(
        text,
        Style::default()
            .fg(color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ))
    .render(area, buf);
}

fn render_choice(text: &str, area: Rect, buf: &mut Buffer, color: Color, selected: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = if selected {
        Style::default()
            .fg(color)
            .bg(FOCUS_BG)
            .add_modifier(Modifier::BOLD)
    } else if color == Color::LightGreen {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Blue)
    };
    for y in area.y..area.y.saturating_add(area.height) {
        buf.set_line(
            area.x,
            y,
            &Line::styled(" ".repeat(area.width as usize), style),
            area.width,
        );
    }
    Paragraph::new(tui_markdown::from_str(text))
        .style(style)
        .wrap(Wrap { trim: false })
        .render(area, buf);
}

fn render_buttons(
    area: Rect,
    buf: &mut Buffer,
    focused_button: Option<RemoteJournalConflictButton>,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(10),
            Constraint::Length(1),
            Constraint::Length(14),
        ])
        .split(area);
    render_button(
        "続行",
        cols[1],
        buf,
        focused_button == Some(RemoteJournalConflictButton::Continue),
    );
    render_button(
        "キャンセル",
        cols[3],
        buf,
        focused_button == Some(RemoteJournalConflictButton::Cancel),
    );
}

fn render_button(text: &'static str, area: Rect, buf: &mut Buffer, focused: bool) {
    let style = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(Color::White)
    };
    Paragraph::new(Line::styled(format!(" {text} "), style))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(style),
        )
        .render(area, buf);
}

fn markdown_line_count(text: &str, width: u16) -> usize {
    if width == 0 {
        0
    } else {
        Paragraph::new(tui_markdown::from_str(text)).line_count(width)
    }
}

fn button_x(button: RemoteJournalConflictButton, width: u16) -> u16 {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(10),
            Constraint::Length(1),
            Constraint::Length(14),
        ])
        .split(Rect::new(0, 0, width, 3));
    match button {
        RemoteJournalConflictButton::Continue => cols[1].x.saturating_add(1),
        RemoteJournalConflictButton::Cancel => cols[3].x.saturating_add(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_remote_journal_conflict_widget_renders_side_by_side_choices() {
        let widget = RemoteJournalConflictWidget::new(
            "# Local\n\n- edited item",
            "# Server\n\n- remote item",
            RemoteJournalConflictChoice::Local,
        );
        render_snapshot(
            "remote_journal_conflict_widget_side_by_side_choices",
            120,
            24,
            widget,
        );
    }

    #[test]
    fn line_count_uses_taller_side_of_comparison() {
        let one_line = RemoteJournalConflictWidget::new(
            "local notes",
            "server notes",
            RemoteJournalConflictChoice::Local,
        );
        let multi_line = RemoteJournalConflictWidget::new(
            "local notes\n\nline two\nline three",
            "server notes",
            RemoteJournalConflictChoice::Local,
        );
        assert_eq!(one_line.line_count(80), 8);
        assert!(multi_line.line_count(80) > one_line.line_count(80));
    }
}
