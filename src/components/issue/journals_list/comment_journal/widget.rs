use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;
use ratatui::prelude::{Line, Span, Stylize};
use ratatui::widgets::{Paragraph, Widget, Wrap};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct CommentJournalWidgetState {
    buffer: Buffer,
    hash: u64,
}

impl CommentJournalWidgetState {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
            hash: 0,
        }
    }

    pub fn update(
        &mut self,
        width: u16,
        creator: &String,
        updated_at: &DateTime<Local>,
        body: &String,
    ) {
        let mut hasher = DefaultHasher::new();
        creator.hash(&mut hasher);
        updated_at.hash(&mut hasher);
        body.hash(&mut hasher);
        let hash = hasher.finish();

        if self.buffer.area.width != width || self.hash != hash {
            self.buffer = Self::render_in_buffer(width, creator, updated_at, body);
            self.hash = hash;
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        let width = min(self.buffer.area.width, area.width);
        let height = min(self.buffer.area.height, area.height);

        for y in 0..height {
            for x in 0..width {
                let Some(src_cell) = self.buffer.cell((x, y)).cloned() else {
                    continue;
                };
                let dst_x = area.x + x;
                let dst_y = area.y + y;
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    *dst_cell = src_cell;
                }
            }
        }

        if focused {
            for y in 0..height {
                for x in 0..width {
                    if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                        cell.set_bg(FOCUS_BG);
                    }
                }
            }
        }
    }

    pub fn line_count(&self, _: u16) -> u16 {
        self.buffer.area.height
    }

    pub fn body_line_count(&self, _: u16) -> u16 {
        self.buffer.area.height.saturating_sub(2)
    }

    fn render_in_buffer(
        width: u16,
        creator: &String,
        updated_at: &DateTime<Local>,
        body: &String,
    ) -> Buffer {
        let header = create_comment_header(creator, updated_at);
        let body = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
        let body_line_count = body.line_count(width) as u16;
        let area = Rect::new(0, 0, width, 2 + body_line_count);
        let mut buffer = Buffer::empty(area);
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Fill(1)])
            .split(area);
        header.render(areas[0], &mut buffer);
        body.render(areas[1], &mut buffer);
        buffer
    }
}

#[derive(Clone)]
pub struct CommentJournalWidget<'a> {
    state: &'a CommentJournalWidgetState,
    focused: bool,
}

impl<'a> CommentJournalWidget<'a> {
    pub fn new(state: &'a CommentJournalWidgetState, focused: bool) -> Self {
        Self { state, focused }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.state.line_count(width)
    }

    pub fn body_line_count(&self, width: u16) -> u16 {
        self.state.body_line_count(width)
    }
}

impl Widget for CommentJournalWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.state.render(area, buf, self.focused);
    }
}

fn create_comment_header(creator: &String, updated_at: &DateTime<Local>) -> Line<'static> {
    Line::from(vec![
        Span::from(creator.clone()).blue(),
        Span::from("が"),
        Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
        Span::from("に更新"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};

    #[test]
    fn snapshot_comment_journal_markdown_wrap() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let body =
            "comment body with **bold**, *italic*, and [link](https://example.com) that wraps."
                .to_string();
        let mut state = CommentJournalWidgetState::new();
        state.update(20, &creator, &updated_at, &body);
        render_snapshot(
            "comment_journal_markdown_wrap",
            20,
            state.line_count(20),
            CommentJournalWidget::new(&state, true),
        );
    }

    #[test]
    fn line_count_comment_journal_current_values() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let body = "comment body with **markdown** and a second sentence that wraps.".to_string();
        let mut state = CommentJournalWidgetState::new();
        state.update(20, &creator, &updated_at, &body);
        let widget = CommentJournalWidget::new(&state, false);
        assert_eq!(widget.body_line_count(20), 4);
        assert_eq!(widget.line_count(20), 6);
    }

    #[test]
    fn line_count_comment_journal_changes_with_width_and_body() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let short_body =
            "Comment with enough words to wrap in a narrow area.".to_string();
        let long_body =
            "Comment with enough words to wrap in a narrow area and then continue for additional wrapped lines."
                .to_string();
        let mut state = CommentJournalWidgetState::new();

        state.update(32, &creator, &updated_at, &short_body);
        let wide_short = CommentJournalWidget::new(&state, false).line_count(32);

        state.update(18, &creator, &updated_at, &short_body);
        let narrow_short = CommentJournalWidget::new(&state, false).line_count(18);

        state.update(18, &creator, &updated_at, &long_body);
        let narrow_long = CommentJournalWidget::new(&state, false).line_count(18);

        assert!(wide_short < narrow_short);
        assert!(narrow_short < narrow_long);
    }
}
