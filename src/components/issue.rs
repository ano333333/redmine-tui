use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use super::Component;

pub struct IssueComponent {
    pub id: u16,
    pub title: String,
}

impl IssueComponent {
    fn create_paragraph(&self) -> Paragraph<'_> {
        Paragraph::new(vec![
            Line::from(format!("#{}", self.id)),
            Line::from(""),
            Line::from(format!("# {}", self.title)).style(Style::default().bold()),
        ])
        .wrap(Wrap { trim: true })
    }
}

impl Component for IssueComponent {
    fn line_count(&self, width: u16) -> u16 {
        // FIXME: widthから占有行数を計算する
        let p = self.create_paragraph();
        p.line_count(width) as u16
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(self.create_paragraph(), area);
    }
}
