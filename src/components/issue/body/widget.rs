use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::{Paragraph, Widget, Wrap};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct BodyWidgetState {
    buffer: Buffer,
}

impl BodyWidgetState {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
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

    pub fn line_count(&self, _: u16) -> usize {
        self.buffer.area.height as usize
    }

    pub fn update(&mut self, width: u16, body: &String) {
        self.buffer = Self::render_in_buffer(body, width);
    }

    fn render_in_buffer(body: &String, width: u16) -> Buffer {
        let paragraph = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
        let area = Rect::new(0, 0, width, paragraph.line_count(width) as u16);
        let mut buffer = Buffer::empty(area);
        paragraph.render(area, &mut buffer);
        buffer
    }
}

pub struct BodyWidget<'a> {
    state: &'a BodyWidgetState,
    focused: bool,
}

impl<'a> Widget for BodyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.state.render(area, buf, self.focused);
    }
}

impl<'a> BodyWidget<'a> {
    pub fn new(state: &'a BodyWidgetState, focused: bool) -> Self {
        Self { state, focused }
    }
    pub fn line_count(&self, width: u16) -> usize {
        self.state.line_count(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_body_markdown_wide() {
        let mut state = BodyWidgetState::new();
        let body = "# Heading\n\n- first item with **bold**\n- second item with *italic*\n\nParagraph with [link](https://example.com).".to_string();
        state.update(32, &body);
        render_snapshot("body_markdown_wide", 32, 8, BodyWidget::new(&state, true));
    }

    #[test]
    fn snapshot_body_markdown_narrow_wrap() {
        let mut state = BodyWidgetState::new();
        let body = "# Heading\n\n- first item with **bold**\n- second item with *italic*\n\nParagraph text with [link](https://example.com) that should wrap.".to_string();
        state.update(18, &body);
        render_snapshot("body_markdown_narrow_wrap", 18, 10, BodyWidget::new(&state, false));
    }

    #[test]
    fn line_count_body_current_values() {
        let mut state = BodyWidgetState::new();
        let body =
            "# Heading\n\n- first item\n- second item\n\nParagraph text that should wrap."
                .to_string();
        state.update(32, &body);
        assert_eq!(BodyWidget::new(&state, false).line_count(32), 6);

        state.update(18, &body);
        assert_eq!(BodyWidget::new(&state, false).line_count(18), 7);
    }

    #[test]
    fn line_count_body_changes_with_width_and_body() {
        let mut state = BodyWidgetState::new();
        let short_body =
            "Paragraph with enough words to wrap once in a narrow area.".to_string();
        let long_body = "Paragraph with enough words to wrap once in a narrow area, then expand into several additional wrapped lines for height growth.".to_string();

        state.update(32, &short_body);
        let wide_short = BodyWidget::new(&state, false).line_count(32);

        state.update(16, &short_body);
        let narrow_short = BodyWidget::new(&state, false).line_count(16);

        state.update(16, &long_body);
        let narrow_long = BodyWidget::new(&state, false).line_count(16);

        assert!(wide_short < narrow_short);
        assert!(narrow_short < narrow_long);
    }
}
