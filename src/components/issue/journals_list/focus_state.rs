use crossterm::event::Event;
use ratatui::layout::{Offset, Position};

use super::comment_journal::focus_state::FocusState as CommentFocusState;
use super::property_journal::focus_state::FocusState as PropertyFocusState;

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

pub enum ChildFocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum ChildEventProcessResult {
    CursorLeavedFromBelow,
    CursorLeavedFromAbove,
}

pub enum ItemFocusState {
    Comment(CommentFocusState),
    Property(PropertyFocusState),
}

pub enum ItemMetrics {
    Comment { width: u16, body_line_count: u16 },
    Property { line_count: u16 },
}

pub struct ItemUpdate {
    pub id: u16,
    pub line_count: u16,
    pub metrics: ItemMetrics,
    pub focus_state: ItemFocusState,
}

pub struct FocusState {
    items: Vec<ItemUpdate>,
    focused_id: Option<u16>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            items: vec![],
            focused_id: None,
        }
    }

    pub fn update(&mut self, new_items: Vec<ItemUpdate>) {
        let focused_id = self.focused_id;
        let old_items = std::mem::take(&mut self.items);
        let mut items = Vec::with_capacity(new_items.len());

        for mut new_item in new_items {
            if let Some(old_item) = old_items.iter().find(|old_item| old_item.id == new_item.id) {
                match (&mut new_item.focus_state, &old_item.focus_state) {
                    (ItemFocusState::Comment(new_state), ItemFocusState::Comment(old_state)) => {
                        *new_state = old_state.clone();
                    }
                    (ItemFocusState::Property(new_state), ItemFocusState::Property(old_state)) => {
                        *new_state = old_state.clone();
                    }
                    _ => {}
                }
            }
            match (&mut new_item.focus_state, &new_item.metrics) {
                (
                    ItemFocusState::Comment(state),
                    ItemMetrics::Comment {
                        width,
                        body_line_count,
                    },
                ) => state.update(*width, *body_line_count),
                (ItemFocusState::Property(state), ItemMetrics::Property { line_count }) => {
                    state.update(*line_count)
                }
                _ => {}
            }
            items.push(new_item);
        }

        self.items = items;
        self.focused_id = focused_id.and_then(|id| {
            if self.items.iter().any(|item| item.id == id) {
                Some(id)
            } else {
                None
            }
        });
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        if let Some(old_index) = self.focused_index() {
            self.items[old_index]
                .focus_state
                .focus_event(ChildFocusEvent::Unfocused);
        }

        match event {
            FocusEvent::Focused { position } => {
                let mut line_count_sum = 0;
                let mut new_index = None;
                for (index, item) in self.items.iter().enumerate() {
                    if line_count_sum + item.line_count >= position.y {
                        new_index = Some(index);
                        break;
                    }
                    line_count_sum += item.line_count;
                }
                self.focused_id = new_index.map(|index| self.items[index].id);
                if let Some(index) = new_index {
                    self.items[index].focus_state.focus_event(ChildFocusEvent::Focused {
                        position: Position::new(position.x, position.y.saturating_sub(line_count_sum)),
                    });
                }
            }
            FocusEvent::Unfocused => {
                self.focused_id = None;
            }
            FocusEvent::CursorEnteredFromAbove => {
                self.focused_id = self.items.first().map(|item| item.id);
                if let Some(index) = self.focused_index() {
                    self.items[index]
                        .focus_state
                        .focus_event(ChildFocusEvent::CursorEnteredFromAbove);
                }
            }
            FocusEvent::CursorEnteredFromBelow { x: _ } => {
                self.focused_id = self.items.last().map(|item| item.id);
                if let Some(index) = self.focused_index() {
                    self.items[index]
                        .focus_state
                        .focus_event(ChildFocusEvent::CursorEnteredFromBelow);
                }
            }
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Option<EventProcessResult> {
        let focused_index = self.focused_index()?;
        let result = self.items[focused_index].focus_state.process_event(event)?;

        match result {
            ChildEventProcessResult::CursorLeavedFromBelow => {
                if focused_index + 1 < self.items.len() {
                    self.items[focused_index]
                        .focus_state
                        .focus_event(ChildFocusEvent::Unfocused);
                    self.focused_id = Some(self.items[focused_index + 1].id);
                    self.items[focused_index + 1]
                        .focus_state
                        .focus_event(ChildFocusEvent::CursorEnteredFromAbove);
                    None
                } else {
                    Some(EventProcessResult::CursorLeavedFromBelow)
                }
            }
            ChildEventProcessResult::CursorLeavedFromAbove => {
                if focused_index > 0 {
                    self.items[focused_index]
                        .focus_state
                        .focus_event(ChildFocusEvent::Unfocused);
                    self.focused_id = Some(self.items[focused_index - 1].id);
                    self.items[focused_index - 1]
                        .focus_state
                        .focus_event(ChildFocusEvent::CursorEnteredFromBelow);
                    None
                } else {
                    Some(EventProcessResult::CursorLeavedFromAbove)
                }
            }
        }
    }

    pub fn focused_index(&self) -> Option<usize> {
        let focused_id = self.focused_id?;
        self.items.iter().position(|item| item.id == focused_id)
    }

    pub fn get_cursor_position(&self) -> Position {
        let Some(index) = self.focused_index() else {
            return Position::default();
        };
        let offset_y = self
            .items
            .iter()
            .take(index)
            .map(|item| item.line_count as i32)
            .sum();
        self.items[index].focus_state.get_cursor_position() + Offset { x: 0, y: offset_y }
    }
}

impl ItemFocusState {
    pub fn focus_event(&mut self, event: ChildFocusEvent) {
        match self {
            ItemFocusState::Comment(state) => state.focus_event(event),
            ItemFocusState::Property(state) => state.focus_event(event),
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Option<ChildEventProcessResult> {
        match self {
            ItemFocusState::Comment(state) => state.process_event(event),
            ItemFocusState::Property(state) => state.process_event(event),
        }
    }

    pub fn get_cursor_position(&self) -> Position {
        match self {
            ItemFocusState::Comment(state) => state.get_cursor_position(),
            ItemFocusState::Property(state) => state.get_cursor_position(),
        }
    }
}

impl Clone for ItemFocusState {
    fn clone(&self) -> Self {
        match self {
            ItemFocusState::Comment(state) => ItemFocusState::Comment(state.clone()),
            ItemFocusState::Property(state) => ItemFocusState::Property(state.clone()),
        }
    }
}
