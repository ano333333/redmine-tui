use std::cmp::min;
use std::collections::HashSet;

use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::{Offset, Position, Rect};

use crate::app::Store;
use crate::entities::Journal;

use super::comment_journal::CommentJournalComponent;
use super::focus_events::FocusEvent as ChildFocusEvent;
use super::journals_list_item::JournalsListItem;
use super::process_event_results::EventProcessResult as ChildEventProcessResult;
use super::property_journal::PropertyJournalComponent;

pub struct JournalsListComponent {
    pub id: u16,
    journals: Vec<Box<dyn JournalsListItem>>,
    focused_index: Option<usize>,
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

impl JournalsListComponent {
    pub fn new(issue_id: u16) -> Self {
        Self {
            id: issue_id,
            journals: vec![],
            focused_index: None,
        }
    }

    pub fn process_event(
        &mut self,
        event: &Event,
        store: &Store,
        width: u16,
    ) -> Option<EventProcessResult> {
        if let Some(index) = self.focused_index {
            let result = self.journals[index].process_event(event, store, width);
            match result {
                Some(ChildEventProcessResult::CursorLeavedFromBelow) => {
                    if index + 1 < self.journals.len() {
                        self.journals[index].focus_event(store, ChildFocusEvent::Unfocused, width);
                        self.focused_index = Some(index + 1);
                        self.journals[index + 1].focus_event(
                            store,
                            ChildFocusEvent::CursorEnteredFromAbove,
                            width,
                        );
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromBelow);
                    }
                }
                Some(ChildEventProcessResult::CursorLeavedFromAbove) => {
                    if index > 0 {
                        self.journals[index].focus_event(store, ChildFocusEvent::Unfocused, width);
                        self.focused_index = Some(index - 1);
                        self.journals[index - 1].focus_event(
                            store,
                            ChildFocusEvent::CursorEnteredFromBelow,
                            width,
                        );
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove);
                    }
                }
                None => {}
            }
        }
        None
    }

    pub fn focus_event(&mut self, store: &Store, event: FocusEvent, width: u16) {
        match event {
            FocusEvent::Focused { position } => {
                let mut line_count_sum: u16 = 0;
                let mut new_focused_index: Option<usize> = None;
                for (index, journal) in self.journals.iter_mut().enumerate() {
                    let line_count = journal.line_count(store, width);
                    if line_count_sum + line_count >= position.y {
                        new_focused_index = Some(index);
                        break;
                    } else {
                        line_count_sum += line_count;
                    }
                }
                if let Some(new_focused_index) = new_focused_index {
                    if let Some(old_focused_index) = self.focused_index {
                        self.journals[old_focused_index].focus_event(
                            store,
                            ChildFocusEvent::Unfocused,
                            width,
                        );
                    }
                    self.focused_index = Some(new_focused_index);
                    let position = Position {
                        x: position.x,
                        y: position.y - line_count_sum,
                    };
                    self.journals[new_focused_index].focus_event(
                        store,
                        ChildFocusEvent::Focused { position },
                        width,
                    );
                }
            }
            FocusEvent::Unfocused => {
                if let Some(prev_focused) = self.focused_index {
                    self.journals[prev_focused].focus_event(
                        store,
                        ChildFocusEvent::Unfocused,
                        width,
                    );
                    self.focused_index = None;
                }
            }
            FocusEvent::CursorEnteredFromAbove => {
                // FIXME:
                // 子が1つもない場合の処理、focus_eventがさらに値を返せるようにしないといけない
                if !self.journals.is_empty() {
                    self.focused_index = Some(0);
                    self.journals[0].focus_event(
                        store,
                        ChildFocusEvent::CursorEnteredFromAbove,
                        width,
                    );
                }
            }
            FocusEvent::CursorEnteredFromBelow { .. } => {
                // FIXME:
                // 子が1つもない場合の処理、focus_eventがさらに値を返せるようにしないといけない
                if !self.journals.is_empty() {
                    let new_index = self.journals.len() - 1;
                    self.focused_index = Some(new_index);
                    self.journals[new_index].focus_event(
                        store,
                        ChildFocusEvent::CursorEnteredFromBelow,
                        width,
                    );
                }
            }
        }
    }

    pub fn update(&mut self, store: &Store, width: u16) -> Option<EventProcessResult> {
        if let Some(issue) = store.get_issue(self.id) {
            for journal in &mut self.journals {
                journal.update(store, width);
            }

            let mut journal_ids_after = HashSet::<u16>::new();
            for id in &issue.journal_ids {
                journal_ids_after.insert(*id);
            }

            let mut focused_id: Option<u16> = None;
            if let Some(index) = self.focused_index {
                focused_id = Some(self.journals[index].get_id());
                if journal_ids_after
                    .get(&self.journals[index].get_id())
                    .is_none()
                {
                    self.journals[index].focus_event(store, ChildFocusEvent::Unfocused, width);
                }
            }
            self.journals
                .retain(|journal| journal_ids_after.get(&journal.get_id()).is_some());

            for index in 0..issue.journal_ids.len() {
                let id_after = issue.journal_ids[index];
                if index >= self.journals.len() || self.journals[index].get_id() != id_after {
                    let journal = store.get_journal(id_after);
                    if let Some(Journal::Property { .. }) = journal {
                        self.journals
                            .insert(index, Box::new(PropertyJournalComponent::new(id_after)));
                    } else if let Some(Journal::Comment { .. }) = journal {
                        self.journals
                            .insert(index, Box::new(CommentJournalComponent::new(id_after)));
                    }
                }
            }

            if let Some(index) = &mut self.focused_index {
                if self.journals.is_empty() {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                if *index >= self.journals.len() {
                    *index = self.journals.len() - 1;
                }
                if let Some(focused_id) = focused_id
                    && self.journals[*index].get_id() != focused_id
                {
                    if let Some(focused) = self
                        .journals
                        .iter_mut()
                        .filter(|journal| journal.get_id() == focused_id)
                        .next()
                    {
                        focused.focus_event(store, ChildFocusEvent::Unfocused, width);
                    }
                    self.journals[*index].focus_event(
                        store,
                        ChildFocusEvent::Focused {
                            position: Position::new(0, 0),
                        },
                        width,
                    );
                }
            }
        }
        None
    }

    pub fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        let line_counts = self
            .journals
            .iter()
            .map(|journal| journal.line_count(store, max_width));
        let line_counts_sum = line_counts.clone().sum();
        let mut buffer_area = Rect::new(0, 0, max_width, min(line_counts_sum, max_height));
        let mut buffer = Buffer::empty(buffer_area);
        for (journal, c) in self.journals.iter().zip(line_counts) {
            if let Some(src) = journal.render(store, max_width, buffer_area.height) {
                render_buffer_to_frame(&mut buffer, &mut buffer_area, &src, src.area);
            }
        }
        Some(buffer)
    }

    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        self.journals
            .iter()
            .map(|journal| journal.line_count(store, width))
            .sum()
    }

    pub fn get_cursor_position(&self, store: &Store, width: u16) -> Position {
        if let Some(index) = self.focused_index {
            let mut sum: i32 = 0;
            for i in 0..index {
                sum += self.journals[i].line_count(store, width) as i32;
            }
            self.journals[index].get_cursor_position() + Offset { x: 0, y: sum }
        } else {
            Position::default()
        }
    }
}

/// BufferをBuffer先頭にコピーし、コピー先の書き込んだ領域を切り詰める
///
/// # Arguments
///
/// * `dst` - コピー先のBuffer
/// * `dst_area` - `dst`の領域
/// * `src` - コピー元のBuffer
/// * `src_area` - `src`の領域
fn render_buffer_to_frame(dst: &mut Buffer, dst_area: &mut Rect, src: &Buffer, src_area: Rect) {
    let width = dst_area.width.min(src_area.width);
    let height = dst_area.height.min(src_area.height);

    for y in 0..height {
        for x in 0..width {
            let Some(src_cell) = src.cell((src_area.x + x, src_area.y + y)).cloned() else {
                continue;
            };
            let dst_x = dst_area.x + x;
            let dst_y = dst_area.y + y;
            if let Some(dst_cell) = dst.cell_mut((dst_x, dst_y)) {
                *dst_cell = src_cell;
            }
        }
    }

    dst_area.y += height;
    dst_area.height -= height;
}
