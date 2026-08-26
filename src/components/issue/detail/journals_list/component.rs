use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::Journal;

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
            // TODO: 次のコミットでEditRequestedとしてEventProcessResultに伝播する
            ChildEventProcessResult::EditRequested { .. } => None,
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
