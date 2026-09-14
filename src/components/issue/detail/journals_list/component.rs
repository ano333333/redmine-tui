use crossterm::event::Event;
use ratatui::layout::Position;

use crate::stores::{LocalJournalEntry, RemoteJournalEntry, Store};
use crate::vos::{EntityIdValue, IssueId, JournalId};

use super::journals_list_item::EventProcessResult as ChildEventProcessResult;
use super::journals_list_item::FocusEvent as ChildFocusEvent;
use super::journals_list_item::JournalsListItemComponent;
use super::local_journal_item::LocalJournalItemComponent;
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
    SaveRequested { id: JournalId },
}

pub struct JournalsListComponent {
    issue_id: IssueId,
    focused_id: Option<u16>,
    items: Vec<JournalsListItemComponent>,
    local_item: Option<LocalJournalItemComponent>,
    width: u16,
}

impl JournalsListComponent {
    pub fn new(issue_id: IssueId) -> Self {
        Self {
            issue_id,
            focused_id: None,
            items: vec![],
            local_item: None,
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
            ChildEventProcessResult::SaveRequested { id } => {
                Some(EventProcessResult::SaveRequested { id })
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

    pub fn update(
        &mut self,
        entries: &[RemoteJournalEntry],
        local_entry: Option<&LocalJournalEntry>,
        width: u16,
    ) -> Option<EventProcessResult> {
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
        for (index, entry) in entries.iter().enumerate() {
            let journal_id = entry.journal.id;
            // 新しいJournalが末尾以外に追加することはないと考え、
            // journalがself.items[index]に来るまでself.itemsの要素を間引く
            while index < self.items.len() && journal_id.get() != self.items[index].id {
                self.items.remove(index);
            }
            if index >= self.items.len() {
                self.items
                    .push(JournalsListItemComponent::new(self.issue_id, journal_id));
            }
            self.items[index].update(entry, width);
        }

        self.items.truncate(entries.len());
        self.local_item = local_entry.map(|entry| {
            let mut component = self
                .local_item
                .take()
                .unwrap_or_else(LocalJournalItemComponent::new);
            component.update(entry, width);
            component
        });

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

    pub fn create_widget<'a>(&'a self, store: &'a Store) -> JournalsListWidget<'a> {
        let mut widgets = self
            .items
            .iter()
            .map(|component| component.create_widget(store))
            .collect::<Vec<_>>();
        if let Some(local_item) = &self.local_item {
            // Local JournalはRemote Journalの時系列には属さないため、常にRemote一覧の末尾へ置く。
            widgets.push(local_item.create_widget());
        }
        JournalsListWidget::new(widgets)
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.items
            .iter()
            .map(|component| component.line_count(width))
            .sum::<u16>()
            + self
                .local_item
                .as_ref()
                .map_or(0, |item| item.line_count(width))
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
            line_count += component.line_count(width);
        }
        Position::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::stores::RemoteJournalState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::entities::Journal;
    use crate::entities::LocalJournal;
    use crate::stores::{
        Action, JournalAction, LocalJournalEntry, LocalJournalState, RemoteJournalEntry, Store,
    };
    use crate::test_support::local_datetime;
    use crate::test_support::render_snapshot;
    use crate::vos::{IssueId, JournalId};

    const WIDE_WIDTH: u16 = 32;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl_s_event() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
    }

    fn entry_state(store: &Store) -> &RemoteJournalState {
        &store.get_remote_journal(1, 1).state
    }

    fn create_journal(id: u16, notes: impl Into<String>) -> Journal {
        Journal {
            id: JournalId::new(id),
            issue_id: IssueId::new(1),
            user: "alice".to_string(),
            updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
            details: vec![],
            notes: notes.into(),
        }
    }

    fn register_journal<'s>(store: &'s mut Store, journal: &Journal) -> &'s RemoteJournalEntry {
        store.consume_action(Action::Journal(JournalAction::SyncFetched {
            issue_id: journal.issue_id,
            journals: vec![journal.clone()],
        }));
        store.get_remote_journal(journal.issue_id, journal.id)
    }

    #[test]
    fn process_event_e_on_focused_item_returns_edit_requested() {
        let mut store = Store::new();
        let journal = create_journal(1, "first paragraph");
        register_journal(&mut store, &journal);
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            None,
            WIDE_WIDTH,
        );
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

    #[test]
    fn process_event_ctrl_s_on_edited_focused_item_returns_save_requested() {
        let mut store = Store::new();
        let journal = create_journal(1, "first paragraph");
        register_journal(&mut store, &journal);
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            None,
            WIDE_WIDTH,
        );
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        store.consume_action(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: journal.issue_id,
            journal_id: journal.id,
            notes: "edited notes".to_string(),
        }));
        component.update(
            store.get_remote_journals(journal.issue_id),
            None,
            WIDE_WIDTH,
        );

        let result = component.process_event(ctrl_s_event());

        match result {
            Some(EventProcessResult::SaveRequested { id }) => {
                assert_eq!(id, JournalId::new(1));
            }
            _ => panic!("expected save request"),
        }
        assert!(matches!(
            entry_state(&store),
            RemoteJournalState::Edited { .. }
        ));
    }

    #[test]
    fn process_event_ctrl_s_on_synced_focused_item_is_a_no_op() {
        let mut store = Store::new();
        let journal = create_journal(1, "first paragraph");
        register_journal(&mut store, &journal);
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            None,
            WIDE_WIDTH,
        );
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(ctrl_s_event());

        assert!(result.is_none());
    }

    #[test]
    fn process_event_ctrl_s_on_unfocused_list_is_a_no_op() {
        let mut component = JournalsListComponent::new(IssueId::new(1));

        let result = component.process_event(ctrl_s_event());

        assert!(result.is_none());
    }

    #[test]
    fn snapshot_local_only_item_is_after_remote_items() {
        let mut store = Store::new();
        let journal = create_journal(1, "remote notes");
        register_journal(&mut store, &journal);
        store.consume_action(Action::Journal(JournalAction::CreateLocal {
            issue_id: journal.issue_id,
        }));
        store.consume_action(Action::Journal(JournalAction::EditLocalNotes {
            issue_id: journal.issue_id,
            notes: "local notes".to_string(),
        }));
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            store.get_local_journal(journal.issue_id),
            WIDE_WIDTH,
        );
        let widget = component.create_widget(&store);
        let line_count = widget.line_count(WIDE_WIDTH);

        render_snapshot(
            "journals_list_local_only_item_after_remote_items",
            WIDE_WIDTH,
            line_count,
            widget,
        );
    }

    #[test]
    fn snapshot_uploading_local_item() {
        let store = Store::new();
        let local_entry = LocalJournalEntry {
            journal: LocalJournal {
                issue_id: IssueId::new(1),
                notes: "uploading local notes".to_string(),
            },
            state: LocalJournalState::Uploading,
        };
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        let widget = component.create_widget(&store);
        let line_count = widget.line_count(WIDE_WIDTH);

        render_snapshot(
            "journals_list_local_item_uploading",
            WIDE_WIDTH,
            line_count,
            widget,
        );
    }
}
