use crossterm::event::{Event, KeyCode};
use ratatui::layout::Position;

pub enum FocusEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
}

pub struct FocusState {
    ids: Vec<u16>,
    focused_id: Option<u16>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            ids: vec![],
            focused_id: None,
        }
    }

    pub fn update(&mut self, ids: &[u16]) {
        let Some(focused_id) = self.focused_id else {
            self.ids = ids.to_vec();
            return;
        };

        let new_index = if let Some(pos) = ids.iter().position(|&id| id == focused_id) {
            // フォーカス中のIDがまだ存在する -> そのインデックスを維持
            pos
        } else {
            // なくなっていれば同じインデックス（末尾にclamp）
            let old_index = self
                .ids
                .iter()
                .position(|&id| id == focused_id)
                .unwrap_or(0);
            old_index.min(ids.len().saturating_sub(1))
        };

        self.ids = ids.to_vec();
        self.focused_id = ids.get(new_index).copied();
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        match event {
            FocusEvent::Unfocused => {
                self.focused_id = None;
            }
            FocusEvent::CursorEnteredFromAbove => {
                self.focused_id = self.ids.first().copied();
            }
            FocusEvent::CursorEnteredFromBelow => {
                self.focused_id = self.ids.last().copied();
            }
        }
    }

    pub fn process_event(&mut self, event: &Event) -> Option<EventProcessResult> {
        let focused_id = self.focused_id?;
        let Event::Key(key) = event else {
            return None;
        };
        let focused_index = self.ids.iter().position(|&id| id == focused_id)?;

        match key.code {
            KeyCode::Char('j') => {
                if focused_index + 1 == self.ids.len() {
                    return Some(EventProcessResult::CursorLeavedFromBelow);
                }
                self.focused_id = self.ids.get(focused_index + 1).copied();
            }
            KeyCode::Char('k') => {
                if focused_index == 0 {
                    return Some(EventProcessResult::CursorLeavedFromAbove);
                }
                self.focused_id = self.ids.get(focused_index - 1).copied();
            }
            _ => {}
        }
        None
    }

    pub fn get_cursor_position(&self) -> Position {
        let focused_id = self.focused_id.unwrap();
        let index = self
            .ids
            .iter()
            .position(|&id| id == focused_id)
            .unwrap_or(0);
        Position {
            x: 0,
            y: index as u16 + 2,
        }
    }
}
