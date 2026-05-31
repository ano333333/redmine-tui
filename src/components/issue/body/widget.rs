use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub struct BodyWidgetState {
    buffer: Buffer,
    hash: u64,
}

impl BodyWidgetState {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
            hash: 0,
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
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
    }

    pub fn line_count(&self, _: u16) -> usize {
        self.buffer.area.height as usize
    }

    pub fn update(&mut self, width: u16, body: &String) {
        if self.buffer.area.width != width {
            self.buffer = Self::render_in_buffer(body, width);
            return;
        }
        let mut hasher = DefaultHasher::new();
        body.hash(&mut hasher);
        if self.hash != hasher.finish() {
            self.buffer = Self::render_in_buffer(body, width);
        }
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
}

impl<'a> Widget for BodyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.state.render(area, buf);
    }
}

impl<'a> BodyWidget<'a> {
    pub fn new(state: &'a BodyWidgetState) -> Self {
        Self { state }
    }
    pub fn line_count(&self, width: u16) -> usize {
        self.state.line_count(width)
    }
}
