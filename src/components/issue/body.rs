use std::cmp::min;

use crossterm::event::{Event, KeyCode};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueBodyComponent {
    id: u16,
    cursor_position: Option<Position>,
}

pub enum FocusEvent {
    Focused { x: u16, y: u16 },
    Unfocused,
    CursorEnteredFromAbove { x: u16 },
    CursorEnteredFromBelow { x: u16 },
}

#[derive(Clone, Copy)]
pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
}

impl IssueBodyComponent {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            cursor_position: None,
        }
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

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some(issue) = store.get_issue(self.id) {
            let body = create_widgets(issue);
            body.line_count(width) as u16
        } else {
            0
        }
    }

    pub fn update(&mut self, store: &Store, width: u16) {
        let height = self.line_count(store, width);
        if let Some(Position { x, y }) = &mut self.cursor_position {
            *x = min(*x, width - 1);
            *y = min(*y, height - 1);
        }
    }

    pub fn focus_event(&mut self, event: FocusEvent, store: &Store, width: u16) {
        match event {
            FocusEvent::Focused { x, y } => {
                self.cursor_position = Some(Position { x, y });
            }
            FocusEvent::Unfocused => {
                self.cursor_position = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                self.cursor_position = Some(Position { x, y: 0 });
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                self.cursor_position = Some(Position {
                    x,
                    y: self.line_count(store, width).saturating_sub(1),
                });
            }
        }
    }

    pub fn process_event(
        &mut self,
        event: &Event,
        store: &Store,
        width: u16,
    ) -> Option<EventProcessResult> {
        if let Some(cursor) = self.cursor_position
            && let Event::Key(key) = event
        {
            match key.code {
                KeyCode::Char('h') => {
                    self.cursor_position = Some(Position {
                        x: cursor.x.saturating_sub(1),
                        y: cursor.y,
                    });
                }
                KeyCode::Char('l') => {
                    self.cursor_position = Some(Position {
                        x: min(cursor.x + 1, width - 1),
                        y: cursor.y,
                    });
                }
                KeyCode::Char('j') => {
                    if cursor.y + 1 == self.line_count(store, width) {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: cursor.x });
                    }
                    self.cursor_position = Some(Position {
                        x: cursor.x,
                        y: cursor.y + 1,
                    });
                }
                KeyCode::Char('k') => {
                    if cursor.y == 0 {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: cursor.x });
                    }
                    self.cursor_position = Some(Position {
                        x: cursor.x,
                        y: cursor.y - 1,
                    });
                }
                _ => {}
            }
        }
        None
    }

    pub fn get_cursor_position(&self) -> Position {
        self.cursor_position.unwrap()
    }
}

// FIXME:
// tui_markdownのレンダリングを、Widget単位ではなくレンダリング結果単位で永続化する方法を考える
// Bufferの永続化？
fn create_widgets(issue: &Issue) -> Paragraph<'_> {
    Paragraph::new(tui_markdown::from_str(&issue.body)).wrap(Wrap { trim: true })
}
