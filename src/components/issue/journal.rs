use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

use chrono::{DateTime, Local};
use crossterm::event::{Event, KeyCode};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::app::{Dispatcher, Store};
use crate::entities::Journal;

pub struct JournalComponent {
    pub id: u16,
    cursor_position: Option<CursorPosition>,
}

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow { x: u16 },
}

pub enum EventProcessResult {
    CursorLeavedFromBelow,
    CursorLeavedFromAbove,
}

enum CursorPosition {
    Header,
    Body { position: Position },
}

impl JournalComponent {
    pub fn new(_: Rc<RefCell<Dispatcher>>, journal_id: u16) -> Self {
        Self {
            id: journal_id,
            cursor_position: None,
        }
    }

    pub fn update(&mut self, _: Rc<RefCell<Dispatcher>>, store: &Store, width: u16) {
        if self.cursor_position.is_some() {
            if let Some(Journal::Comment { body, .. }) = store.get_journal(self.id) {
                let body = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
                let line_count = 2 + body.line_count(width) as u16;
                if line_count > 0
                    && let Some(CursorPosition::Body { position }) = &mut self.cursor_position
                {
                    if position.x >= width {
                        position.x = width - 1;
                    }
                    if position.y >= line_count {
                        position.y = line_count - 1;
                    }
                } else {
                    self.cursor_position = Some(CursorPosition::Header);
                }
            }
        }
    }

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

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        let journal = store.get_journal(self.id);
        if let Some(crate::entities::Journal::Property { .. }) = journal {
            4
        } else if let Some(crate::entities::Journal::Comment { body, .. }) = journal {
            let body = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
            2 + body.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn focus_event(&mut self, store: &Store, event: FocusEvent, width: u16) {
        match event {
            FocusEvent::Focused { position } => {
                let journal = store.get_journal(self.id);
                if let Some(Journal::Property { .. }) = journal {
                    self.cursor_position = Some(CursorPosition::Header);
                } else if let Some(Journal::Comment { .. }) = journal {
                    self.cursor_position = Some(if position.y < 2 {
                        CursorPosition::Header
                    } else {
                        CursorPosition::Body {
                            position: position + Offset { x: 0, y: -2 },
                        }
                    });
                }
            }
            FocusEvent::Unfocused => {
                self.cursor_position = None;
            }
            FocusEvent::CursorEnteredFromAbove => {
                self.cursor_position = Some(CursorPosition::Header);
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                let journal = store.get_journal(self.id);
                if let Some(Journal::Property { .. }) = journal {
                    self.cursor_position = Some(CursorPosition::Header);
                } else if let Some(Journal::Comment { body, .. }) = journal {
                    let body =
                        Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
                    let line_count = 2 + body.line_count(width) as u16;
                    if line_count == 0 {
                        self.cursor_position = Some(CursorPosition::Header);
                    } else {
                        self.cursor_position = Some(CursorPosition::Body {
                            position: Position {
                                x,
                                y: line_count - 1,
                            },
                        });
                    }
                }
            }
        }
    }

    pub fn process_event(
        &mut self,
        event: &Event,
        store: &Store,
        width: u16,
    ) -> Option<EventProcessResult> {
        if let Event::Key(key) = event {
            if let Some(Journal::Property { .. }) = store.get_journal(self.id)
                && let Some(CursorPosition::Header) = self.cursor_position
            {
                if key.code == KeyCode::Char('j') {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                } else if key.code == KeyCode::Char('k') {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
            } else if let Some(Journal::Comment { body, .. }) = store.get_journal(self.id) {
                if let Some(CursorPosition::Header) = self.cursor_position {
                    if key.code == KeyCode::Char('j') {
                        self.cursor_position = Some(CursorPosition::Body {
                            position: Position { x: 0, y: 0 },
                        });
                    } else if key.code == KeyCode::Char('k') {
                        return Some(EventProcessResult::CursorLeavedFromAbove);
                    }
                } else if let Some(CursorPosition::Body { position }) = &mut self.cursor_position {
                    if key.code == KeyCode::Char('j') {
                        let body =
                            Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
                        let line_count = 2 + body.line_count(width) as u16;
                        if position.y + 1 >= line_count {
                            return Some(EventProcessResult::CursorLeavedFromBelow);
                        } else {
                            position.y += 1;
                        }
                    } else if key.code == KeyCode::Char('k') {
                        if position.y > 0 {
                            position.y -= 1;
                        } else {
                            self.cursor_position = Some(CursorPosition::Header);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn get_cursor_position(&self) -> Position {
        if let Some(CursorPosition::Header) = self.cursor_position {
            Position { x: 0, y: 0 }
        } else if let Some(CursorPosition::Body { position }) = self.cursor_position {
            position + Offset { x: 0, y: 2 }
        } else {
            Position::default()
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
