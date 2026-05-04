use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueBodyComponent {
    id: u16,
}

impl IssueBodyComponent {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
    pub fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        if let Some(issue) = store.get_issue(self.id) {
            let body = create_widgets(issue);
            let area = Rect::new(
                0,
                0,
                max_width,
                min(body.line_count(max_width) as u16, max_height),
            );
            let mut buffer = Buffer::empty(area);
            body.render(area, &mut buffer);
            Some(buffer)
        } else {
            None
        }
    }
}

// FIXME:
// tui_markdownのレンダリングを、Widget単位ではなくレンダリング結果単位で永続化する方法を考える
// Bufferの永続化？
fn create_widgets(issue: &Issue) -> Paragraph<'_> {
    Paragraph::new(tui_markdown::from_str(&issue.body)).wrap(Wrap { trim: true })
}
