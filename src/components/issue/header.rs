use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueHeaderComponent {
    id: u16,
}

impl IssueHeaderComponent {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
    pub fn render(&self, store: &Store, frame: &mut Frame, area: &mut Rect) {
        if let Some(issue) = store.get_issue(self.id) {
            let paragraphs = create_widgets(issue);
            for p in paragraphs.iter() {
                frame.render_widget(p, *area);
                let l = p.line_count(area.width) as u16;
                area.y += l;
                area.height = area.height.saturating_sub(l);
            }
        }
    }
}

fn create_widgets(issue: &Issue) -> Vec<Paragraph<'static>> {
    vec![Paragraph::new(vec![
        Line::from(format!("#{}", issue.id)),
        Line::from(""),
        Line::from(format!("# {}", issue.title.clone())).style(Style::default().bold()),
        Line::from("\n"),
        Line::from(vec![
            Span::from(issue.creator.clone()).style(Style::default().blue()),
            Span::from("が"),
            Span::from(issue.appended_at.format("%Y/%m/%d").to_string())
                .style(Style::default().blue()),
            Span::from("に追加. "),
            Span::from(issue.updated_at.format("%Y/%m/%d").to_string())
                .style(Style::default().blue()),
            Span::from("に更新."),
        ]),
        Line::from("\n\n"),
    ])]
}
