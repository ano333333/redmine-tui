use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueBodyComponent {
    id: u16,
}

impl IssueBodyComponent {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
    pub fn render(&self, store: &Store, frame: &mut Frame, area: &mut Rect) {
        if let Some(issue) = store.get_issue(self.id) {
            let body = create_widgets(issue);
            frame.render_widget(&body, *area);
            let l = body.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
    }
}

// FIXME:
// tui_markdownのレンダリングを、Widget単位ではなくレンダリング結果単位で永続化する方法を考える
// Bufferの永続化？
fn create_widgets(issue: &Issue) -> Paragraph<'_> {
    Paragraph::new(tui_markdown::from_str(&issue.body)).wrap(Wrap { trim: true })
}
