use crossterm::event::Event;
use ratatui::layout::Position;

use crate::stores::Store;
use crate::vos::JournalKey;

use super::journals_list_item::EventProcessResult as ChildEventProcessResult;
use super::journals_list_item::FocusEvent as ChildFocusEvent;
use super::journals_list_item::{JournalItemContent, JournalsListItemComponent};
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
    EditRequested { key: JournalKey, notes: String },
}

pub struct JournalsListComponent {
    focused_key: Option<JournalKey>,
    items: Vec<JournalsListItemComponent>,
    width: u16,
}

impl JournalsListComponent {
    pub fn new() -> Self {
        Self {
            focused_key: None,
            items: vec![],
            width: 0,
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let focused_key = self.focused_key?;
        let result = self
            .items
            .iter_mut()
            .find(|component| component.key == focused_key)?
            .process_event(event)?;

        match result {
            ChildEventProcessResult::CursorLeavedFromBelow { x } => {
                let focused_index = self
                    .items
                    .iter()
                    .position(|journal| journal.key == focused_key)?;
                if focused_index + 1 < self.items.len() {
                    self.items[focused_index].focus_event(ChildFocusEvent::Unfocused);
                    self.focused_key = Some(self.items[focused_index + 1].key);
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
                    .position(|journal| journal.key == focused_key)?;
                if focused_index > 0 {
                    self.items[focused_index].focus_event(ChildFocusEvent::Unfocused);
                    self.focused_key = Some(self.items[focused_index - 1].key);
                    self.items[focused_index - 1]
                        .focus_event(ChildFocusEvent::CursorEnteredFromBelow { x });
                    None
                } else {
                    Some(EventProcessResult::CursorLeavedFromAbove)
                }
            }
            ChildEventProcessResult::EditRequested { key, notes } => {
                Some(EventProcessResult::EditRequested { key, notes })
            }
        }
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        if let Some(old_key) = self.focused_key
            && let Some(component) = self
                .items
                .iter_mut()
                .find(|component| component.key == old_key)
        {
            component.focus_event(ChildFocusEvent::Unfocused);
        }

        match event {
            FocusEvent::Focused { position } => {
                let mut line_count_sum = 0;
                let mut new_index = None;
                for (index, component) in self.items.iter().enumerate() {
                    let line_count = component.line_count(self.width);
                    if line_count_sum + line_count > position.y {
                        new_index = Some(index);
                        break;
                    }
                    line_count_sum += line_count;
                }
                let Some(new_index) = new_index else {
                    return;
                };
                self.focused_key = Some(self.items[new_index].key);
                self.items[new_index].focus_event(ChildFocusEvent::Focused {
                    position: Position::new(position.x, position.y - line_count_sum),
                });
            }
            FocusEvent::Unfocused => {
                self.focused_key = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                // FIXME: journalsが1個もない場合の処理
                // 「フォーカスが当たらず下に通り抜ける」
                if let Some(focused_key) = self.focused_key
                    && let Some(component) = self
                        .items
                        .iter_mut()
                        .find(|component| component.key == focused_key)
                {
                    component.focus_event(ChildFocusEvent::Unfocused);
                }
                let Some(component) = self.items.first_mut() else {
                    return;
                };
                self.focused_key = Some(component.key);
                component.focus_event(ChildFocusEvent::CursorEnteredFromAbove { x });
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                // FIXME: journalsが1個もない場合の処理
                // 「フォーカスが当たらず上に通り抜ける」
                if let Some(focused_key) = self.focused_key
                    && let Some(component) = self
                        .items
                        .iter_mut()
                        .find(|component| component.key == focused_key)
                {
                    component.focus_event(ChildFocusEvent::Unfocused);
                }
                let Some(component) = self.items.last_mut() else {
                    return;
                };
                self.focused_key = Some(component.key);
                component.focus_event(ChildFocusEvent::CursorEnteredFromBelow { x });
            }
        }
    }

    pub fn update(
        &mut self,
        journals: &[(JournalKey, JournalItemContent)],
        width: u16,
    ) -> Option<EventProcessResult> {
        // Storeの不変条件: Local Journalは0または1件、存在するなら末尾の
        // journal_keysだけ。ここより前の並べ替えは行わず、違反はpanicする。
        let local_indexes: Vec<usize> = journals
            .iter()
            .enumerate()
            .filter_map(|(index, (_, content))| {
                matches!(content, JournalItemContent::Local(_)).then_some(index)
            })
            .collect();
        if local_indexes.len() > 1
            || local_indexes
                .iter()
                .any(|&index| index != journals.len() - 1)
        {
            panic!(
                "Local Journal must be at most one and only the last journal_keys entry (Store invariant violated)"
            );
        }

        self.width = width;
        // 今フォーカスが当たっているJournalKeyとそのインデックスを保持する。
        // もしそのJournalが削除されていたら、同じ位置(範囲外なら末尾)のJournalに
        // フォーカスを当て、Journalが0件ならフォーカスを解除する
        let focused_key = self.focused_key;
        let old_focus_index = focused_key
            .and_then(|key| self.items.iter().position(|component| component.key == key));

        // keyが一致するitemはそのまま引き継ぎ、一致しない順に新しいitemを作る
        let mut updated_items = Vec::with_capacity(journals.len());
        for (key, content) in journals {
            match self
                .items
                .iter()
                .position(|component| component.key == *key)
            {
                Some(index) => {
                    let mut component = self.items.remove(index);
                    component.update(content, width);
                    updated_items.push(component);
                }
                None => {
                    let mut component = JournalsListItemComponent::new(*key, content);
                    component.update(content, width);
                    updated_items.push(component);
                }
            }
        }
        self.items = updated_items;

        if let Some(focused_key) = focused_key
            && self
                .items
                .iter()
                .find(|component| component.key == focused_key)
                .is_none()
        {
            match old_focus_index {
                Some(index) if !self.items.is_empty() => {
                    let index = index.min(self.items.len() - 1);
                    self.items[index].focus_event(ChildFocusEvent::Focused {
                        position: Position { x: 0, y: 0 },
                    });
                    self.focused_key = Some(self.items[index].key);
                }
                _ => {
                    self.focused_key = None;
                }
            }
        }
        None
    }

    pub fn create_widget<'a>(&'a self, store: &Store) -> JournalsListWidget<'a> {
        JournalsListWidget::new(
            self.items
                .iter()
                .map(|component| component.create_widget(store))
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
        let Some(focused_key) = self.focused_key else {
            return Position::new(0, 0);
        };
        let mut line_count = 0;
        for component in self.items.iter() {
            if component.key == focused_key {
                let mut position = component.get_cursor_position();
                position.y += line_count;
                return position;
            }
            line_count += component.line_count(width);
        }
        Position::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::entities::{Journal, LocalJournal};
    use crate::test_support::local_datetime;
    use crate::vos::{EntityIdValue, JournalId, JournalKey, LocalJournalId};

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

    fn remote_key(id: u16) -> JournalKey {
        JournalKey::Remote(JournalId::new(id))
    }

    fn update_component(component: &mut JournalsListComponent, journals: &[&Journal]) {
        let entries = journals
            .iter()
            .map(|journal| {
                (
                    remote_key(journal.id.get()),
                    JournalItemContent::Remote(*journal),
                )
            })
            .collect::<Vec<_>>();
        component.update(&entries, WIDE_WIDTH);
    }

    fn create_local_journal(id: u64, notes: impl Into<String>) -> LocalJournal {
        LocalJournal {
            id: LocalJournalId::new(id),
            issue_id: 3.into(),
            notes: notes.into(),
        }
    }

    fn update_with_remote_and_local(
        component: &mut JournalsListComponent,
        remote: &Journal,
        local: &LocalJournal,
    ) {
        let entries = vec![
            (
                remote_key(remote.id.get()),
                JournalItemContent::Remote(remote),
            ),
            (
                JournalKey::Local(local.id),
                JournalItemContent::Local(local),
            ),
        ];
        component.update(&entries, WIDE_WIDTH);
    }

    /// 先頭itemから`j`をpresses回押してpresses個下のitemへフォーカスを移す
    fn press_j(component: &mut JournalsListComponent, presses: usize) {
        for _ in 0..presses {
            assert!(
                component
                    .process_event(key_event(KeyCode::Char('j')))
                    .is_none()
            );
        }
    }

    #[test]
    fn process_event_e_on_focused_item_returns_edit_requested_with_journal_key() {
        let journal = create_journal(7, "first paragraph");
        let mut component = JournalsListComponent::new();
        update_component(&mut component, &[&journal]);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested { key, notes }) => {
                assert_eq!(key, JournalKey::Remote(JournalId::new(7)));
                assert_eq!(notes, "first paragraph");
            }
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    fn update_keeps_focus_on_same_item_when_key_survives_reorder() {
        let a = create_journal(1, "a notes");
        let b = create_journal(2, "b notes");
        let c = create_journal(3, "c notes");
        let mut component = JournalsListComponent::new();
        update_component(&mut component, &[&a, &b, &c]);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        press_j(&mut component, 1);
        assert_eq!(component.focused_key, Some(remote_key(2)));

        update_component(&mut component, &[&b, &a, &c]);

        assert_eq!(component.focused_key, Some(remote_key(2)));
        // bはindex 0に移動したが同一itemへfocusが残る(Notes先頭の相対y=3)
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(0, 3)
        );
    }

    #[test]
    fn update_moves_focus_to_same_index_when_focused_item_removed() {
        let a = create_journal(1, "a notes");
        let b = create_journal(2, "b notes");
        let c = create_journal(3, "c notes");
        let mut component = JournalsListComponent::new();
        update_component(&mut component, &[&a, &b, &c]);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        press_j(&mut component, 1);
        assert_eq!(component.focused_key, Some(remote_key(2)));

        update_component(&mut component, &[&a, &c]);

        assert_eq!(component.focused_key, Some(remote_key(3)));
        // fallback先(c, index 1)へFocused eventが適用される
        // (前のitem 4行 + Notes先頭の相対y=3)
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(0, 7)
        );
        // cが最終itemのNotes先頭にfocusしていることをイベントで確認する
        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
    }

    #[test]
    fn update_moves_focus_to_last_item_when_old_index_out_of_range() {
        let a = create_journal(1, "a notes");
        let b = create_journal(2, "b notes");
        let c = create_journal(3, "c notes");
        let mut component = JournalsListComponent::new();
        update_component(&mut component, &[&a, &b, &c]);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        press_j(&mut component, 2);
        assert_eq!(component.focused_key, Some(remote_key(3)));

        update_component(&mut component, &[&a, &b]);

        assert_eq!(component.focused_key, Some(remote_key(2)));
        // 範囲外の旧indexは末尾(b, index 1)へfallbackする
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(0, 7)
        );
        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
    }

    #[test]
    fn update_clears_focus_when_no_items_remain() {
        let a = create_journal(1, "a notes");
        let b = create_journal(2, "b notes");
        let mut component = JournalsListComponent::new();
        update_component(&mut component, &[&a, &b]);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        press_j(&mut component, 1);
        assert_eq!(component.focused_key, Some(remote_key(2)));

        update_component(&mut component, &[]);

        assert_eq!(component.focused_key, None);
        assert!(component.items.is_empty());
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(0, 0)
        );
    }

    #[test]
    fn process_event_e_on_focused_local_returns_edit_requested_with_local_key_and_notes() {
        let remote = create_journal(1, "remote notes");
        let local = create_local_journal(1, "local notes");
        let mut component = JournalsListComponent::new();
        update_with_remote_and_local(&mut component, &remote, &local);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        press_j(&mut component, 1);
        assert_eq!(
            component.focused_key,
            Some(JournalKey::Local(LocalJournalId::new(1)))
        );

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested { key, notes }) => {
                assert_eq!(key, JournalKey::Local(LocalJournalId::new(1)));
                assert_eq!(notes, "local notes");
            }
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    #[should_panic(expected = "Local Journal must be")]
    fn update_panics_when_local_is_not_at_tail() {
        let a = create_journal(1, "a notes");
        let b = create_journal(2, "b notes");
        let local = create_local_journal(1, "local notes");
        let mut component = JournalsListComponent::new();
        let entries = vec![
            (remote_key(1), JournalItemContent::Remote(&a)),
            (
                JournalKey::Local(local.id),
                JournalItemContent::Local(&local),
            ),
            (remote_key(2), JournalItemContent::Remote(&b)),
        ];

        component.update(&entries, WIDE_WIDTH);
    }

    #[test]
    #[should_panic(expected = "Local Journal must be")]
    fn update_panics_when_multiple_locals_exist() {
        let local_1 = create_local_journal(1, "local 1 notes");
        let local_2 = create_local_journal(2, "local 2 notes");
        let mut component = JournalsListComponent::new();
        let entries = vec![
            (
                JournalKey::Local(local_1.id),
                JournalItemContent::Local(&local_1),
            ),
            (
                JournalKey::Local(local_2.id),
                JournalItemContent::Local(&local_2),
            ),
        ];

        component.update(&entries, WIDE_WIDTH);
    }
}
