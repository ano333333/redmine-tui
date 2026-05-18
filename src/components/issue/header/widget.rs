use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct HeaderWidget<'a> {
    pub id: u16,
    pub title: &'a String,
    pub creator: &'a String,
    pub appended_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl<'a> Widget for HeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragraph = Paragraph::new(vec![
            Line::from(format!("#{}", self.id)),
            Line::from(""),
            Line::from(format!("# {}", self.title.clone())).style(Style::default().bold()),
            // FIXME: 改行を指定して2行の間を作ろうとしているが、実際は1行分の空白しかできていない
            Line::from("\n"),
            Line::from(vec![
                Span::from(self.creator.clone()).style(Style::default().blue()),
                Span::from("が"),
                Span::from(self.appended_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に追加. "),
                Span::from(self.updated_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に更新."),
            ]),
            // FIXME: 改行2つで3行の間を作ろうとしているが、実際は1行分の空白しかできていない
            Line::from("\n\n"),
        ]);
        paragraph.render(area, buf);
    }
}

impl<'a> HeaderWidget<'a> {
    pub fn line_count(&self, _: u16) -> usize {
        6
    }
}
