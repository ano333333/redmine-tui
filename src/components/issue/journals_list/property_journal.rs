use std::cmp::min;

use chrono::{DateTime, Local};
use crossterm::event::{Event, KeyCode};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::Store;
use crate::entities::Journal;

use super::focus_events::FocusEvent;
use super::journals_list_item::JournalsListItem;
use super::process_event_results::EventProcessResult;

pub struct PropertyJournalComponent {
    pub id: u16,
    focused: bool,
}

impl PropertyJournalComponent {
    pub fn new(journal_id: u16) -> Self {
        Self {
            id: journal_id,
            focused: false,
        }
    }
}

impl JournalsListItem for PropertyJournalComponent {
    fn get_id(&self) -> u16 {
        self.id
    }

    fn update(&mut self, _: &Store, _: u16) {}

    fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        if let Some(Journal::Property {
            creator,
            target,
            old,
            new,
            updated_at,
            ..
        }) = store.get_journal(self.id)
        {
            let widgets = PropertyWidgets::new(creator, target, old, new, updated_at);
            let area = Rect::new(0, 0, max_width, min(4, max_height));
            let mut buffer = Buffer::empty(area);
            widgets.paragraph.render(area, &mut buffer);
            Some(buffer)
        } else {
            None
        }
    }

    fn line_count(&self, store: &Store, _: u16) -> u16 {
        if let Some(Journal::Property { .. }) = store.get_journal(self.id) {
            4
        } else {
            0
        }
    }

    fn focus_event(&mut self, store: &Store, event: FocusEvent, _: u16) {
        match event {
            FocusEvent::Focused { .. }
            | FocusEvent::CursorEnteredFromAbove
            | FocusEvent::CursorEnteredFromBelow => {
                if let Some(Journal::Property { .. }) = store.get_journal(self.id) {
                    self.focused = true;
                }
            }
            FocusEvent::Unfocused => {
                self.focused = false;
            }
        }
    }

    fn process_event(
        &mut self,
        event: &Event,
        store: &Store,
        _: u16,
    ) -> Option<EventProcessResult> {
        if let Some(Journal::Property { .. }) = store.get_journal(self.id)
            && let Event::Key(key) = event
            && self.focused
        {
            if key.code == KeyCode::Char('j') {
                return Some(EventProcessResult::CursorLeavedFromBelow);
            } else if key.code == KeyCode::Char('k') {
                return Some(EventProcessResult::CursorLeavedFromAbove);
            }
        }
        None
    }

    fn get_cursor_position(&self) -> Position {
        Position { x: 0, y: 0 }
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
