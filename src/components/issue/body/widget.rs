use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub struct BodyWidget<'a> {
    // NOTE: BodyWidgetStateのようなステートを受け取るようにする
    pub body: &'a String,
}

impl<'a> Widget for BodyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragraph = self.create_paragraph();
        paragraph.render(area, buf);
    }
}

impl<'a> BodyWidget<'a> {
    pub fn line_count(&self, width: u16) -> usize {
        let paragraph = self.create_paragraph();
        paragraph.line_count(width)
    }

    fn create_paragraph(&self) -> Paragraph<'a> {
        Paragraph::new(tui_markdown::from_str(self.body)).wrap(Wrap { trim: true })
    }
}
