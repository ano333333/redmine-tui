use crossterm::event::Event;
use crossterm::event::KeyCode;
use ratatui::layout::Offset;
use ratatui::prelude::Stylize;
use ratatui::prelude::Widget;
use std::cmp::min;

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::Store;
use crate::entities::Journal;

use super::focus_events::FocusEvent;
use super::journals_list_item::JournalsListItem;
use super::process_event_results::EventProcessResult;

pub struct CommentJournalComponent {
    pub id: u16,
    cursor_position: Option<CursorPosition>,
}

enum CursorPosition {
    Header,
    Body { position: Position },
}

impl CommentJournalComponent {
    pub fn new(journal_id: u16) -> Self {
        Self {
            id: journal_id,
            cursor_position: None,
        }
    }
}

impl JournalsListItem for CommentJournalComponent {
    fn get_id(&self) -> u16 {
        self.id
    }
    fn update(&mut self, store: &Store, width: u16) {
        if self.cursor_position.is_some()
            && let Some(Journal::Comment { body, .. }) = store.get_journal(self.id)
        {
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

    fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        if let Some(crate::entities::Journal::Comment {
            creator,
            updated_at,
            body,
            ..
        }) = store.get_journal(self.id)
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

    fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some(crate::entities::Journal::Comment { body, .. }) = store.get_journal(self.id) {
            let body = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
            2 + body.line_count(width) as u16
        } else {
            0
        }
    }

    fn focus_event(&mut self, store: &Store, event: FocusEvent, width: u16) {
        match event {
            FocusEvent::Focused { position } => {
                if let Some(Journal::Comment { .. }) = store.get_journal(self.id) {
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
            FocusEvent::CursorEnteredFromBelow => {
                if let Some(Journal::Comment { body, .. }) = store.get_journal(self.id) {
                    let body =
                        Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
                    let line_count = 2 + body.line_count(width) as u16;
                    if line_count == 0 {
                        self.cursor_position = Some(CursorPosition::Header);
                    } else {
                        self.cursor_position = Some(CursorPosition::Body {
                            position: Position {
                                x: 0,
                                y: line_count - 1,
                            },
                        });
                    }
                }
            }
        }
    }

    fn process_event(
        &mut self,
        event: &Event,
        store: &Store,
        width: u16,
    ) -> Option<EventProcessResult> {
        if let Event::Key(key) = event
            && let Some(Journal::Comment { body, .. }) = store.get_journal(self.id)
        {
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
                } else if key.code == KeyCode::Char('h')
                    && let Some(CursorPosition::Body { position }) = &mut self.cursor_position
                    && position.x > 0
                {
                    position.x -= 1;
                } else if key.code == KeyCode::Char('l')
                    && let Some(CursorPosition::Body { position }) = &mut self.cursor_position
                    && position.x + 1 < width
                {
                    position.x += 1;
                }
            }
        }
        None
    }

    fn get_cursor_position(&self) -> Position {
        if let Some(CursorPosition::Header) = self.cursor_position {
            Position { x: 0, y: 0 }
        } else if let Some(CursorPosition::Body { position }) = self.cursor_position {
            position + Offset { x: 0, y: 2 }
        } else {
            Position::default()
        }
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
