use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Line, Span, Stylize};
use ratatui::widgets::{Paragraph, Widget, Wrap};

#[derive(Clone)]
pub struct CommentJournalWidget<'a> {
    creator: &'a String,
    updated_at: &'a DateTime<Local>,
    body: &'a String,
}

impl<'a> CommentJournalWidget<'a> {
    pub fn new(creator: &'a String, updated_at: &'a DateTime<Local>, body: &'a String) -> Self {
        Self {
            creator,
            updated_at,
            body,
        }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        2 + self.body_line_count(width)
    }

    pub fn body_line_count(&self, width: u16) -> u16 {
        Paragraph::new(tui_markdown::from_str(self.body))
            .wrap(Wrap { trim: true })
            .line_count(width) as u16
    }
}

impl Widget for CommentJournalWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let header = create_comment_header(self.creator, self.updated_at);
        let body = Paragraph::new(tui_markdown::from_str(self.body)).wrap(Wrap { trim: true });
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Fill(1)])
            .split(area);
        header.render(areas[0], buf);
        body.render(areas[1], buf);
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
