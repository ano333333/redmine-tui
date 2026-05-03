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

pub enum JournalComponent {
    Property {
        creator: String,
        target: String,
        old: String,
        new: String,
        updated_at: DateTime<Local>,
    },
    Comment {
        creator: String,
        updated_at: DateTime<Local>,
        body: String,
    },
}

impl JournalComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>, journal_id: u16) -> Self {
        let dispatcher = dispatcher.borrow();
        let journal = dispatcher.store().get_journal(journal_id);
        if journal.is_none() {
            panic!();
        }
        let journal = journal.unwrap();
        match journal {
            crate::entities::Journal::Property {
                id,
                creator,
                target,
                old,
                new,
                updated_at,
            } => JournalComponent::Property {
                creator: creator.clone(),
                target: target.clone(),
                old: old.clone(),
                new: new.clone(),
                updated_at: updated_at.clone(),
            },
            crate::entities::Journal::Comment {
                id,
                creator,
                updated_at,
                body,
            } => JournalComponent::Comment {
                creator: creator.clone(),
                updated_at: updated_at.clone(),
                body: body.clone(),
            },
        }
    }
}

impl Component for JournalComponent {
    fn line_count(&self, width: u16) -> u16 {
        match self {
            Self::Property { .. } => 4,
            Self::Comment { body, .. } => {
                let body_text = tui_markdown::from_str(body);
                Paragraph::new(body_text)
                    .wrap(Wrap { trim: true })
                    .line_count(width) as u16
                    + 3
            }
        }
    }

    fn render(&self, _: &Store, frame: &mut Frame, mut area: Rect) {
        match self {
            Self::Property {
                creator,
                target,
                old,
                new,
                updated_at,
            } => {
                let title = Line::from(vec![
                    Span::from(creator).blue(),
                    Span::from("が"),
                    Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
                    Span::from("に更新"),
                ]);
                let body = Line::from(vec![
                    Span::from("  ・ "),
                    Span::from(target).bold(),
                    Span::from(" を "),
                    Span::from(old).italic(),
                    Span::from(" から "),
                    Span::from(new).italic(),
                    Span::from(" に変更"),
                ])
                .gray();
                let margin = Line::from("");
                frame.render_widget(
                    Paragraph::new(Text::from(vec![title, Line::from(""), body, margin])),
                    area,
                );
            }
            Self::Comment {
                creator,
                updated_at,
                body,
            } => {
                let title = Line::from(vec![
                    Span::from(creator).blue(),
                    Span::from("が"),
                    Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
                    Span::from("に更新"),
                ]);
                area.y += 1;
                area.height = area.height.saturating_sub(1);
                let mut body = tui_markdown::from_str(body);
                body.lines.push(Line::from(""));
                frame.render_widget(title, area);
                frame.render_widget(body, area);
            }
        }
    }
}
