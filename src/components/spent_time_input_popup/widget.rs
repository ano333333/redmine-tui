use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui_textarea::TextArea;

use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED, TIMELINE_FG};

pub struct SpentTimeInputPopupWidget<'a> {
    activity: &'a str,
    hours_textarea: &'a TextArea<'a>,
    memo_textarea: &'a TextArea<'a>,
    activity_focused: bool,
    hours_textarea_focused: bool,
    memo_textarea_focused: bool,
    submit_button_focused: bool,
}

impl<'a> SpentTimeInputPopupWidget<'a> {
    pub fn new(
        activity: &'a str,
        hours_textarea: &'a TextArea<'a>,
        memo_textarea: &'a TextArea<'a>,
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

    pub fn popup_area(area: Rect) -> Rect {
        let width = area.width.saturating_mul(4) / 5;
        let height = area.height.saturating_mul(3) / 5;
        Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        }
    }
}

impl Widget for SpentTimeInputPopupWidget<'_> {
    /// クライアント領域に対する描画(したがってClearの責務がある)
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Self::popup_area(area);
        if area.width < 3 || area.height < 3 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 実績工数 ")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(TIMELINE_FG));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Length(3),
            ])
            .split(inner);

        let summary = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(16)])
            .split(rows[0]);

        let activity = Paragraph::new(Line::from(self.activity)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ACTIVITY ")
                .border_style(Self::border_style(self.activity_focused)),
        );
        activity.render(summary[0], buf);

        let mut hours = self.hours_textarea.clone();
        hours.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" HOURS ")
                .border_style(Self::border_style(self.hours_textarea_focused)),
        );
        (&hours).render(summary[1], buf);

        let mut memo = self.memo_textarea.clone();
        memo.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" MEMO ")
                .border_style(Self::border_style(self.memo_textarea_focused)),
        );
        (&memo).render(rows[1], buf);

        let actions = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Length(1),
            ])
            .split(rows[2]);
        Paragraph::new(Line::styled(
            "Enter: edit / h,j,k,l: move",
            Style::default().fg(MUTED),
        ))
        .render(rows[2], buf);
        let button = Paragraph::new(Line::from(" SAVE ")).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Self::border_style(self.submit_button_focused)),
        );
        button.render(actions[1], buf);
    }
}

impl<'a> SpentTimeInputPopupWidget<'a> {
    fn border_style(focused: bool) -> Style {
        if focused {
            Style::default().fg(ACCENT).bg(FOCUS_BG)
        } else {
            Style::default().fg(TIMELINE_FG)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    fn textarea_with_value(value: &str) -> TextArea<'static> {
        let mut textarea = TextArea::default();
        textarea.insert_str(value);
        textarea
    }

    #[test]
    fn snapshot_spent_time_input_popup_wide_short_values_all_unfocused() {
        let hours_textarea = textarea_with_value("1.5");
        let memo_textarea = textarea_with_value("朝会対応");
        let widget = SpentTimeInputPopupWidget::new(
            "開発",
            &hours_textarea,
            &memo_textarea,
            false,
            false,
            false,
            false,
        );

        render_snapshot(
            "spent_time_input_popup_wide_short_values_all_unfocused",
            60,
            30,
            widget,
        );
    }

    #[test]
    fn snapshot_spent_time_input_popup_narrow_long_values_all_focused() {
        let hours_textarea = textarea_with_value("1234567890.25h-long-entry");
        let memo_textarea = textarea_with_value(
            "定例確認と関連チケットの調査メモ。表示幅を超える長文入力で先頭側が見切れる状態。",
        );
        let widget = SpentTimeInputPopupWidget::new(
            "とても長いアクティビティ名で表示幅を超えるケース",
            &hours_textarea,
            &memo_textarea,
            true,
            true,
            true,
            true,
        );

        render_snapshot(
            "spent_time_input_popup_narrow_long_values_all_focused",
            30,
            20,
            widget,
        );
    }
}
