use std::cmp::min;

use crossterm::event::{Event, KeyCode};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueHeaderComponent {
    id: u16,
    focused: bool,
}

pub enum FocusEvent {
    Focused,
    Unfocused,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromBelow,
}

impl IssueHeaderComponent {
    pub fn new(id: u16) -> Self {
        Self { id, focused: false }
    }

    pub fn get_cursor_position(&self) -> Position {
        Position { x: 2, y: 2 }
    }

    pub fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        if let Some(issue) = store.get_issue(self.id) {
            let text = create_widgets(issue);
            let area = Rect::new(0, 0, max_width, min(6, max_height));
            let mut buffer = Buffer::empty(area);
            text.render(area, &mut buffer);
            return Some(buffer);
        }
        None
    }

    // TODO: 描画・line_countをWidgetに切り分けて、CompnoentがWidgetを持つ形にした方が良いかも？
    pub fn line_count(&self, store: &Store) -> u16 {
        if store.get_issue(self.id).is_some() {
            6
        } else {
            0
        }
    }
    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::CursorEnteredFromBelow => {
                self.focused = true;
            }
            FocusEvent::Focused { .. } => {
                self.focused = true;
            }
            FocusEvent::Unfocused => {
                self.focused = false;
            }
        }
    }
    pub fn process_event(&mut self, event: &crossterm::event::Event) -> Option<EventProcessResult> {
        if self.focused
            && let Event::Key(key) = event
            && key.code == KeyCode::Char('j')
        {
            return Some(EventProcessResult::CursorLeavedFromBelow);
        }
        None
    }
}

fn create_widgets(issue: &Issue) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(format!("#{}", issue.id)),
        Line::from(""),
        Line::from(format!("# {}", issue.title.clone())).style(Style::default().bold()),
        // FIXME: 改行を指定して2行の間を作ろうとしているが、実際は1行分の空白しかできていない
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
        // FIXME: 改行2つで3行の間を作ろうとしているが、実際は1行分の空白しかできていない
        Line::from("\n\n"),
    ])
}
