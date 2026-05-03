use std::cell::RefCell;
use std::rc::Rc;

use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{Dispatcher, Store};
use crate::components::Component;

pub struct JournalComponent {
    id: u16,
}

impl JournalComponent {
    pub fn new(_: Rc<RefCell<Dispatcher>>, journal_id: u16) -> Self {
        Self { id: journal_id }
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        let journal = store.get_journal(self.id);
        if let Some(crate::entities::Journal::Property { .. }) = journal {
            4
        } else if let Some(crate::entities::Journal::Comment {
            creator,
            updated_at,
            body,
            ..
        }) = journal
        {
            let widgets = CommentWidgets::new(creator, updated_at, body);
            widgets.line_count(width)
        } else {
            1
        }
    }
}

impl Component for JournalComponent {
    fn update(&mut self, _: Rc<RefCell<Dispatcher>>, _: &Store) {}

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
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
            frame.render_widget(&widgets.paragraph, area);
        } else if let Some(crate::entities::Journal::Comment {
            creator,
            updated_at,
            body,
            ..
        }) = journal
        {
            let widgets = CommentWidgets::new(creator, updated_at, body);
            widgets.render(frame, &mut area);
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

struct CommentWidgets {
    pub title: Line<'static>,
    body: String,
}

impl CommentWidgets {
    pub fn new(creator: &String, updated_at: &DateTime<Local>, body: &String) -> Self {
        Self {
            title: Self::create_title(creator, updated_at),
            body: body.clone(),
        }
    }
    fn create_title(creator: &String, updated_at: &DateTime<Local>) -> Line<'static> {
        Line::from(vec![
            Span::from(creator.clone()).blue(),
            Span::from("が"),
            Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
            Span::from("に更新"),
        ])
    }
    // FIXME:
    // tui_markdownのレンダリングを、Widget単位ではなくレンダリング結果単位で永続化する方法を考える
    // Bufferの永続化？
    fn create_body(&self) -> Text {
        let mut text = tui_markdown::from_str(&self.body);
        text.lines.push(Line::from(""));
        text
    }
    fn line_count(&self, width: u16) -> u16 {
        let body_text = self.create_body();
        Paragraph::new(body_text)
            .wrap(Wrap { trim: true })
            .line_count(width) as u16
            + 3
    }
    pub fn render(&self, frame: &mut Frame, area: &mut Rect) {
        frame.render_widget(&self.title, *area);
        area.y += 2;
        area.height = area.height.saturating_sub(2);
        let mut body = self.create_body();
        body.lines.push(Line::from(""));
        frame.render_widget(body, *area);
    }
}
