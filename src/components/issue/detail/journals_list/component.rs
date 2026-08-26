use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::Journal;
use crate::vos::JournalId;

use super::journals_list_item::EventProcessResult as ChildEventProcessResult;
use super::journals_list_item::FocusEvent as ChildFocusEvent;
use super::journals_list_item::JournalsListItemComponent;
use super::widget::JournalsListWidget;

pub enum FocusEvent {
    Focused { position: Position },
    Unfocused,
    CursorEnteredFromAbove { x: u16 },
    CursorEnteredFromBelow { x: u16 },
}

pub enum EventProcessResult {
    CursorLeavedFromBelow,
    CursorLeavedFromAbove,
    EditRequested { id: JournalId, notes: String },
}

pub struct JournalsListComponent {
    focused_id: Option<u16>,
    items: Vec<JournalsListItemComponent>,
    width: u16,
}

impl JournalsListComponent {
    pub fn new() -> Self {
        Self {
            focused_id: None,
            items: vec![],
            width: 0,
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let focused_id = self.focused_id?;
        let result = self
            .items
            .iter_mut()
            .find(|component| component.id == focused_id)?
            .process_event(event)?;

        match result {
            ChildEventProcessResult::CursorLeavedFromBelow { x } => {
                let focused_index = self
                    .items
                    .iter()
                    .enumerate()
                    .find(|(_, journal)| journal.id == focused_id)?
                    .0;
                if focused_index + 1 < self.items.len() {
                    self.items[focused_index].focus_event(ChildFocusEvent::Unfocused);
                    self.focused_id = Some(self.items[focused_index + 1].id);
                    self.items[focused_index + 1]
                        .focus_event(ChildFocusEvent::CursorEnteredFromAbove { x });
                    None
                } else {
                    Some(EventProcessResult::CursorLeavedFromBelow)
                }
            }
            ChildEventProcessResult::CursorLeavedFromAbove { x } => {
                let focused_index = self
                    .items
                    .iter()
                    .enumerate()
                    .find(|(_, journal)| journal.id == focused_id)?
                    .0;
                if focused_index > 0 {
                    self.items[focused_index].focus_event(ChildFocusEvent::Unfocused);
                    self.focused_id = Some(self.items[focused_index - 1].id);
                    self.items[focused_index - 1]
                        .focus_event(ChildFocusEvent::CursorEnteredFromBelow { x });
                    None
                } else {
                    Some(EventProcessResult::CursorLeavedFromAbove)
                }
            }
            ChildEventProcessResult::EditRequested { id, notes } => {
                Some(EventProcessResult::EditRequested { id, notes })
            }
        }
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        if let Some(old_id) = self.focused_id
            && let Some(component) = self
                .items
                .iter_mut()
                .find(|component| component.id == old_id)
        {
            component.focus_event(ChildFocusEvent::Unfocused);
        }

        match event {
            FocusEvent::Focused { position } => {
                let mut line_count_sum = 0;
                let mut new_index = None;
                for (index, component) in self.items.iter().enumerate() {
                    let line_count = component.create_widget().line_count(self.width);
                    if line_count_sum + line_count > position.y {
                        new_index = Some(index);
                        break;
                    }
                    line_count_sum += line_count;
                }
                let Some(new_index) = new_index else {
                    return;
                };
                self.focused_id = Some(self.items[new_index].id);
                self.items[new_index].focus_event(ChildFocusEvent::Focused {
                    position: Position::new(position.x, position.y - line_count_sum),
                });
            }
            FocusEvent::Unfocused => {
                self.focused_id = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                // FIXME: journalsが1個もない場合の処理
                // 「フォーカスが当たらず下に通り抜ける」
                if let Some(focused_id) = self.focused_id
                    && let Some(component) = self
                        .items
                        .iter_mut()
                        .find(|component| component.id == focused_id)
                {
                    component.focus_event(ChildFocusEvent::Unfocused);
                }
                let Some(component) = self.items.first_mut() else {
                    return;
                };
                self.focused_id = Some(component.id);
                component.focus_event(ChildFocusEvent::CursorEnteredFromAbove { x });
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                // FIXME: journalsが1個もない場合の処理
                // 「フォーカスが当たらず上に通り抜ける」
                if let Some(focused_id) = self.focused_id
                    && let Some(component) = self
                        .items
                        .iter_mut()
                        .find(|component| component.id == focused_id)
                {
                    component.focus_event(ChildFocusEvent::Unfocused);
                }
                let Some(component) = self.items.last_mut() else {
                    return;
                };
                self.focused_id = Some(component.id);
                component.focus_event(ChildFocusEvent::CursorEnteredFromBelow { x });
            }
        }
    }

    pub fn update(&mut self, journals: Vec<&Journal>, width: u16) -> Option<EventProcessResult> {
        self.width = width;
        // 今フォーカスが当たっているJournalのself.journalsでのインデックス
        // もしそのJournalが削除されていたら、同じ位置または末尾にあるJournalにフォーカスを当てる
        let mut focused_index = None;
        if let Some(focused_id) = self.focused_id {
            focused_index = self
                .items
                .iter()
                .enumerate()
                .find(|(_, component)| component.id == focused_id)
                .map(|(index, _)| index);
        }
        for (index, journal) in journals.iter().enumerate() {
            // 新しいJournalが末尾以外に追加することはないと考え、
            // journalがself.items[index]に来るまでself.itemsの要素を間引く
            while index < self.items.len() && journal.id != self.items[index].id {
                self.items.remove(index);
            }
            if index >= self.items.len() {
                self.items.push(JournalsListItemComponent::new(journal));
            }
            self.items[index].update(journal, width);
        }

        if let Some(focused_id) = self.focused_id
            && let Some(mut focused_index) = focused_index
            && self.items[focused_index].id != focused_id
            && self
                .items
                .iter()
                .find(|component| component.id == focused_id)
                .is_none()
        {
            if focused_index >= self.items.len() {
                focused_index = self.items.len().saturating_sub(1);
            }
            self.items[focused_index].focus_event(ChildFocusEvent::Focused {
                position: Position { x: 0, y: 0 },
            });
        }
        None
    }

    pub fn create_widget<'a>(&'a self) -> JournalsListWidget<'a> {
        JournalsListWidget::new(
            self.items
                .iter()
                .map(|component| component.create_widget())
                .collect(),
        )
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.items
            .iter()
            .map(|component| component.line_count(width))
            .sum()
    }

    pub fn get_cursor_position(&self, width: u16) -> Position {
        let Some(focused_id) = self.focused_id else {
            return Position::new(0, 0);
        };
        let mut line_count = 0;
        for component in self.items.iter() {
            if component.id == focused_id {
                let mut position = component.get_cursor_position();
                position.y += line_count;
                return position;
            }
            let widget = component.create_widget();
            line_count += widget.line_count(width);
        }
        Position::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::test_support::local_datetime;
    use crate::vos::JournalId;

    const WIDE_WIDTH: u16 = 32;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn create_journal(id: u16, notes: impl Into<String>) -> Journal {
        Journal {
            id: JournalId::new(id),
            user: "alice".to_string(),
            updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
            details: vec![],
            notes: notes.into(),
        }
    }

    #[test]
    fn process_event_e_on_focused_item_returns_edit_requested() {
        let journal = create_journal(1, "first paragraph");
        let mut component = JournalsListComponent::new();
        component.update(vec![&journal], WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested { id, notes }) => {
                assert_eq!(id, JournalId::new(1));
                assert_eq!(notes, "first paragraph");
            }
            _ => panic!("expected edit request"),
        }
    }
}
