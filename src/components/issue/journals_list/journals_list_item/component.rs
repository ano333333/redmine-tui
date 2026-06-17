use std::cmp::min;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

use crate::entities::Journal;

use super::{JournalItemWidget, JournalItemWidgetState};

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove { x: u16 },
    CursorEnteredFromBelow { x: u16 },
}

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
}

enum FocusedPosition {
    Property(usize),
    Comment(Position),
}

pub struct JournalsListItemComponent {
    pub id: u16,
    width: u16,
    journal: Journal,
    comment_line_count: u16,
    focused_position: Option<FocusedPosition>,
    widget_state: JournalItemWidgetState,
}

impl JournalsListItemComponent {
    pub fn new(journal: &Journal) -> Self {
        Self {
            id: journal.id,
            width: 0,
            journal: journal.clone(),
            comment_line_count: 0,
            focused_position: None,
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        if self.focused_position.is_none() {
            return None;
        }
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Char('j') => {
                if let Some(FocusedPosition::Property(index)) = &mut self.focused_position {
                    if *index + 1 < self.journal.details.len() {
                        *index += 1;
                    } else if self.comment_line_count > 0 {
                        self.focused_position =
                            Some(FocusedPosition::Comment(Position { x: 0, y: 0 }));
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: 0 });
                    }
                } else if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                {
                    if position.y + 1 < self.comment_line_count as u16 {
                        position.y += 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromBelow { x: position.x });
                    }
                }
            }
            KeyCode::Char('k') => {
                if let Some(FocusedPosition::Property(index)) = &mut self.focused_position {
                    if *index > 0 {
                        *index -= 1;
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: 0 });
                    }
                } else if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                {
                    if position.y > 0 {
                        position.y -= 1;
                    } else if self.journal.details.len() > 0 {
                        self.focused_position =
                            Some(FocusedPosition::Property(self.journal.details.len() - 1));
                    } else {
                        return Some(EventProcessResult::CursorLeavedFromAbove { x: position.x });
                    }
                }
            }
            KeyCode::Char('h') => {
                if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                    && position.x > 0
                {
                    position.x -= 1;
                }
            }
            KeyCode::Char('l') => {
                if let Some(FocusedPosition::Comment(position)) = &mut self.focused_position
                    && position.x + 1 < self.width
                {
                    position.x += 1;
                }
            }
            _ => {}
        }
        None
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        let property_count = self.journal.details.len();
        match event {
            FocusEvent::Focused { position } => {
                if position.y < property_count as u16 + 2 {
                    self.focused_position = Some(FocusedPosition::Property(min(
                        position.y.saturating_sub(2) as usize,
                        property_count.saturating_sub(1),
                    )));
                } else {
                    self.focused_position = Some(FocusedPosition::Comment(Position {
                        x: min(position.x, self.width.saturating_sub(1)),
                        y: min(
                            position.y - (property_count as u16 + 2),
                            self.comment_line_count,
                        ),
                    }));
                }
            }
            FocusEvent::Unfocused => {
                self.focused_position = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                if property_count > 0 {
                    self.focused_position = Some(FocusedPosition::Property(0));
                } else {
                    self.focused_position = Some(FocusedPosition::Comment(Position { x, y: 0 }));
                }
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                if self.comment_line_count == 0 {
                    self.focused_position =
                        Some(FocusedPosition::Property(property_count.saturating_sub(1)));
                } else {
                    self.focused_position = Some(FocusedPosition::Comment(Position {
                        x,
                        y: self.comment_line_count - 1,
                    }));
                }
            }
        }
    }

    pub fn update(&mut self, journal: &Journal, width: u16) {
        self.width = width;
        self.journal = journal.clone();
        self.widget_state
            .update(width, &journal.user, &journal.updated_on, &journal.notes);
        self.comment_line_count = self.widget_state.comment_line_count();
        let property_count = self.journal.details.len();
        match &mut self.focused_position {
            None => {}
            Some(FocusedPosition::Property(index)) => {
                // FIXME: (現実的かはともかく)今回のupdateでpropertyリストが消えた場合を追加
                if *index >= property_count {
                    *index = property_count.saturating_sub(1);
                }
            }
            Some(FocusedPosition::Comment(position)) => {
                if position.x >= width {
                    position.x = width.saturating_sub(1);
                }
                if position.y >= self.comment_line_count {
                    position.y = self.comment_line_count.saturating_sub(1);
                }
            }
        }
    }

    pub fn create_widget<'a>(&'a self) -> JournalItemWidget<'a> {
        JournalItemWidget::new(
            &self.journal,
            &self.widget_state,
            self.focused_position.is_some(),
        )
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.journal.details.len() as u16 + 1 + self.comment_line_count
    }

    pub fn get_cursor_position(&self) -> Position {
        let property_count = self.journal.details.len();
        match self.focused_position {
            None => Position { x: 0, y: 0 },
            Some(FocusedPosition::Property(index)) => Position {
                x: 0,
                y: index as u16 + 2,
            },
            Some(FocusedPosition::Comment(position)) => Position {
                x: position.x,
                y: 2 + property_count as u16 + 1 + position.y,
            },
        }
    }
}
