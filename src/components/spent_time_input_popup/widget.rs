use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui_textarea::TextArea;

pub struct SpentTimeInputPopupWidget<'a, 'b> {
    activity: &'a str,
    hours_textarea: &'a TextArea<'b>,
    memo_textarea: &'a TextArea<'b>,
    activity_focused: bool,
    hours_textarea_focused: bool,
    memo_textarea_focused: bool,
    submit_button_focused: bool,
}

impl<'a, 'b> SpentTimeInputPopupWidget<'a, 'b> {
    pub fn new(
        activity: &'a str,
        hours_textarea: &'a TextArea<'b>,
        memo_textarea: &'a TextArea<'b>,
        activity_focused: bool,
        hours_textarea_focused: bool,
        memo_textarea_focused: bool,
        submit_button_focused: bool,
    ) -> Self {
        Self {
            activity,
            hours_textarea,
            memo_textarea,
            activity_focused,
            hours_textarea_focused,
            memo_textarea_focused,
            submit_button_focused,
        }
    }
}

impl Widget for SpentTimeInputPopupWidget<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 || area.height < 3 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default().borders(Borders::ALL).title("工数入力");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let cols = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                // アクティビティ
                Constraint::Length(3),
                // 工数
                Constraint::Length(3),
                // メモ
                Constraint::Length(3),
                Constraint::Length(1),
                // 保存ボタン
                Constraint::Length(3),
            ])
            .split(inner);

        let activity = Paragraph::new(Line::from(self.activity)).block(
            Block::default()
                .borders(Borders::ALL)
                .title("アクティビティ")
                .border_style(Self::border_style(self.activity_focused)),
        );
        activity.render(cols[0], buf);

        let mut hours = self.hours_textarea.clone();
        hours.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("工数")
                .border_style(Self::border_style(self.hours_textarea_focused)),
        );
        (&hours).render(cols[1], buf);

        let mut memo = self.memo_textarea.clone();
        memo.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("メモ")
                .border_style(Self::border_style(self.memo_textarea_focused)),
        );
        (&memo).render(cols[2], buf);

        let rows = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Length(1),
            ])
            .split(cols[4]);
        let button = Paragraph::new(Line::from(" 保存 ")).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Self::border_style(self.submit_button_focused)),
        );
        button.render(rows[1], buf);
    }
}

impl<'a, 'b> SpentTimeInputPopupWidget<'a, 'b> {
    fn border_style(focused: bool) -> Style {
        if focused {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default()
        }
    }
}
