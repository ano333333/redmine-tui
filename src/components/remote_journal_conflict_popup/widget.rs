use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const MESSAGE: &str = "Journal本文とサーバー内容が競合しています。採用する値を選択してください。";
const CHOICE_LABELS: [&str; 3] = ["編集前の値", "ローカル値", "サーバー値"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// 競合したJournal本文で採用する値の出所。
pub enum RemoteJournalConflictChoice {
    Before,
    Local,
    Server,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Remote Journal競合popupの操作ボタン。
pub enum RemoteJournalConflictButton {
    Cancel,
    Continue,
}

/// 編集前、ローカル編集後、サーバー現在値を縦に並べる競合解決Widget。
pub struct RemoteJournalConflictWidget {
    notes: [String; 3],
    selected_choice: RemoteJournalConflictChoice,
    focused_button: Option<RemoteJournalConflictButton>,
}

impl RemoteJournalConflictWidget {
    /// 3つの本文と現在の採用値からWidgetを作成する。
    pub fn new(
        before: impl Into<String>,
        local: impl Into<String>,
        server: impl Into<String>,
        selected_choice: RemoteJournalConflictChoice,
    ) -> Self {
        Self {
            notes: [before.into(), local.into(), server.into()],
            selected_choice,
            focused_button: None,
        }
    }

    /// フォーカス表示するボタンを指定したWidgetを返す。
    pub fn with_focused_button(
        mut self,
        focused_button: Option<RemoteJournalConflictButton>,
    ) -> Self {
        self.focused_button = focused_button;
        self
    }

    /// 指定幅で全体を描画するために必要な行数を返す。
    pub fn line_count(&self, width: u16) -> usize {
        if width == 0 {
            return 0;
        }

        let choice_line_count = self
            .notes
            .iter()
            .map(|notes| 1 + notes_line_count(notes, width))
            .sum::<usize>();

        Paragraph::new(MESSAGE)
            .line_count(width)
            .saturating_add(1)
            .saturating_add(choice_line_count)
            .saturating_add(1)
            .saturating_add(3)
    }

    /// 指定した選択肢の仮想バッファ上のカーソル位置を返す。
    pub fn cursor_position_for_choice(&self, choice_index: usize, width: u16) -> Position {
        Position {
            x: 1,
            y: self.choice_notes_y(choice_index, width),
        }
    }

    /// 指定したボタンの仮想バッファ上のカーソル位置を返す。
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

    fn choice_notes_y(&self, choice_index: usize, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }

        let before = self
            .notes
            .iter()
            .take(choice_index)
            .map(|notes| 1 + notes_line_count(notes, width))
            .sum::<usize>();
        Paragraph::new(MESSAGE)
            .line_count(width)
            .saturating_add(1)
            .saturating_add(before)
            .saturating_add(1) as u16
    }

    fn choice_at(&self, choice_index: usize) -> RemoteJournalConflictChoice {
        match choice_index {
            0 => RemoteJournalConflictChoice::Before,
            1 => RemoteJournalConflictChoice::Local,
            2 => RemoteJournalConflictChoice::Server,
            _ => panic!("choice index must be 0..=2"),
        }
    }

    fn notes_line_count(&self, choice_index: usize, width: u16) -> usize {
        notes_line_count(&self.notes[choice_index], width)
    }
}

impl Widget for RemoteJournalConflictWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let message_line_count = Paragraph::new(MESSAGE).line_count(area.width) as u16;
        let constraints = [
            Constraint::Length(message_line_count),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(self.notes_line_count(0, area.width) as u16),
            Constraint::Length(1),
            Constraint::Length(self.notes_line_count(1, area.width) as u16),
            Constraint::Length(1),
            Constraint::Length(self.notes_line_count(2, area.width) as u16),
            Constraint::Length(1),
            Constraint::Length(3),
        ];
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        Paragraph::new(MESSAGE)
            .wrap(Wrap { trim: false })
            .render(areas[0], buf);

        for choice_index in 0..3 {
            let focused = self.choice_at(choice_index) == self.selected_choice;
            render_choice(
                CHOICE_LABELS[choice_index],
                &self.notes[choice_index],
                areas[2 + choice_index * 2],
                areas[3 + choice_index * 2],
                focused,
                buf,
            );
        }

        if let Some(button_area) = areas.last() {
            render_buttons(*button_area, buf, self.focused_button);
        }
    }
}

fn render_choice(
    label: &str,
    notes: &str,
    label_area: Rect,
    notes_area: Rect,
    focused: bool,
    buf: &mut Buffer,
) {
    if label_area.width > 0 && label_area.height > 0 {
        Paragraph::new(Line::styled(
            label,
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .wrap(Wrap { trim: false })
        .render(label_area, buf);
    }

    if notes_area.width > 0 && notes_area.height > 0 {
        let style = focus_style(focused);
        for y in notes_area.y..notes_area.y.saturating_add(notes_area.height) {
            let blank = " ".repeat(notes_area.width as usize);
            buf.set_line(
                notes_area.x,
                y,
                &Line::styled(blank, style),
                notes_area.width,
            );
        }
        Paragraph::new(tui_markdown::from_str(notes))
            .style(style)
            .wrap(Wrap { trim: false })
            .render(notes_area, buf);
    }
}

fn render_buttons(
    area: Rect,
    buf: &mut Buffer,
    focused_button: Option<RemoteJournalConflictButton>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(1),
            Constraint::Length(10),
        ])
        .split(area);

    render_button(
        "キャンセル",
        cols[1],
        buf,
        focused_button == Some(RemoteJournalConflictButton::Cancel),
    );
    render_button(
        "続行",
        cols[3],
        buf,
        focused_button == Some(RemoteJournalConflictButton::Continue),
    );
}

fn render_button(text: &'static str, area: Rect, buf: &mut Buffer, focused: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    Paragraph::new(Line::from(text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(button_border_style(focused)),
        )
        .render(area, buf);
}

fn notes_line_count(notes: &str, width: u16) -> usize {
    if width == 0 {
        return 0;
    }
    Paragraph::new(tui_markdown::from_str(notes)).line_count(width)
}

fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default().bg(FOCUS_BG)
    } else {
        Style::default()
    }
}

fn button_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default()
    }
}

fn button_x(button: RemoteJournalConflictButton, width: u16) -> u16 {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(1),
            Constraint::Length(10),
        ])
        .split(Rect::new(0, 0, width, 3));

    match button {
        RemoteJournalConflictButton::Cancel => cols[1].x.saturating_add(1),
        RemoteJournalConflictButton::Continue => cols[3].x.saturating_add(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_remote_journal_conflict_widget_renders_message_choices_and_buttons() {
        let widget = RemoteJournalConflictWidget::new(
            "# Before\n\n- old item",
            "# Local\n\n- edited item",
            "# Server\n\n- remote item",
            RemoteJournalConflictChoice::Local,
        )
        .with_focused_button(None);

        render_snapshot(
            "remote_journal_conflict_widget_full_area_with_message_choices_and_buttons",
            120,
            28,
            widget,
        );
    }

    #[test]
    fn line_count_includes_message_choice_labels_and_buttons() {
        let widget = RemoteJournalConflictWidget::new(
            "before notes",
            "local notes",
            "server notes",
            RemoteJournalConflictChoice::Local,
        );

        assert_eq!(widget.line_count(80), 12);
    }

    #[test]
    fn line_count_grows_with_multiline_notes() {
        let one_line = RemoteJournalConflictWidget::new(
            "before notes",
            "local notes",
            "server notes",
            RemoteJournalConflictChoice::Local,
        );
        let multi_line = RemoteJournalConflictWidget::new(
            "before notes\n\nline two\nline three",
            "local notes",
            "server notes",
            RemoteJournalConflictChoice::Local,
        );

        assert!(multi_line.line_count(80) > one_line.line_count(80));
    }
}
