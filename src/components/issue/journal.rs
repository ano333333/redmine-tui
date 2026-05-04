use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::app::{Dispatcher, Store};

pub struct JournalComponent {
    id: u16,
}

impl JournalComponent {
    pub fn new(_: Rc<RefCell<Dispatcher>>, journal_id: u16) -> Self {
        Self { id: journal_id }
    }

    pub fn update(&mut self, _: Rc<RefCell<Dispatcher>>, _: &Store) {}

    pub fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        let journal = store.get_journal(self.id);
        if let Some(crate::entities::Journal::Property {
            creator,
            target,
            old,
            new,
            updated_at,
            ..
        }) = journal
        {
            let widgets = PropertyWidgets::new(creator, target, old, new, updated_at);
            let area = Rect::new(0, 0, max_width, min(4, max_height));
            let mut buffer = Buffer::empty(area);
            widgets.paragraph.render(area, &mut buffer);
            Some(buffer)
        } else if let Some(crate::entities::Journal::Comment {
            creator,
            updated_at,
            body,
            ..
        }) = journal
        {
            let header = create_comment_header(creator, updated_at);
            let body = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
            let line_count = 2 + body.line_count(max_width) as u16;
            let area = Rect::new(0, 0, max_width, min(line_count, max_height));
            let mut buffer = Buffer::empty(area);
            let area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Fill(1)])
                .split(area);
            header.render(area[0], &mut buffer);
            body.render(area[1], &mut buffer);
            Some(buffer)
        } else {
            None
        }
    }
}

struct PropertyWidgets<'a> {
    pub paragraph: Paragraph<'a>,
}

impl PropertyWidgets<'_> {
    pub fn new(
        creator: &String,
        target: &String,
        old: &String,
        new: &String,
        updated_at: &DateTime<Local>,
    ) -> Self {
        Self {
            paragraph: Self::create_paragraph(creator, target, old, new, updated_at),
        }
    }
    fn create_paragraph(
        creator: &String,
        target: &String,
        old: &String,
        new: &String,
        updated_at: &DateTime<Local>,
    ) -> Paragraph<'static> {
        let title = Line::from(vec![
            Span::from(creator.clone()).blue(),
            Span::from("が"),
            Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
            Span::from("に更新"),
        ]);
        let body = Line::from(vec![
            Span::from("  ・ "),
            Span::from(target.clone()).bold(),
            Span::from(" を "),
            Span::from(old.clone()).italic(),
            Span::from(" から "),
            Span::from(new.clone()).italic(),
            Span::from(" に変更"),
        ])
        .gray();
        let margin = Line::from("");
        Paragraph::new(Text::from(vec![title, Line::from(""), body, margin]))
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
