use crossterm::event::Event;
use ratatui::layout::Position;

use crate::stores::{LocalJournalEntry, RemoteJournalEntry, Store};
use crate::vos::{EntityIdValue, IssueId, JournalId};

use super::journals_list_item::EventProcessResult as ChildEventProcessResult;
use super::journals_list_item::FocusEvent as ChildFocusEvent;
use super::journals_list_item::JournalsListItemComponent;
use super::local_journal_item::EventProcessResult as LocalEventProcessResult;
use super::local_journal_item::LocalJournalItemComponent;
use super::widget::JournalsListWidget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// RemoteとLocalを同じ一覧でfocusするための、component内だけで使うidentity。
/// Local Journalにdomain IDを発行する代わりには使用しない。
enum JournalItemIdentity {
    Remote(JournalId),
    Local,
}

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
    EditLocalJournalRequested { notes: String },
    CreateLocalJournalRequested,
    SaveLocalJournalRequested,
    SaveRequested { id: JournalId },
}

pub struct JournalsListComponent {
    issue_id: IssueId,
    focused_item: Option<JournalItemIdentity>,
    create_button_focused: bool,
    items: Vec<JournalsListItemComponent>,
    local_item: Option<LocalJournalItemComponent>,
    width: u16,
}

impl JournalsListComponent {
    pub fn new(issue_id: IssueId) -> Self {
        Self {
            issue_id,
            focused_item: None,
            create_button_focused: false,
            items: vec![],
            local_item: None,
            width: 0,
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        if self.create_button_focused {
            return match event {
                Event::Key(key)
                    if key.code == crossterm::event::KeyCode::Enter
                        && self.local_item.is_none() =>
                {
                    Some(EventProcessResult::CreateLocalJournalRequested)
                }
                Event::Key(key) if key.code == crossterm::event::KeyCode::Char('k') => {
                    let Some(item) = self
                        .item_count()
                        .checked_sub(1)
                        .and_then(|index| self.item_identity_at(index))
                    else {
                        return Some(EventProcessResult::CursorLeavedFromAbove);
                    };
                    self.create_button_focused = false;
                    self.focused_item = Some(item);
                    self.focus_item(item, ChildFocusEvent::CursorEnteredFromBelow { x: 0 });
                    None
                }
                Event::Key(key) if key.code == crossterm::event::KeyCode::Char('j') => {
                    Some(EventProcessResult::CursorLeavedFromBelow)
                }
                _ => None,
            };
        }
        let focused_item = self.focused_item?;
        let result = match focused_item {
            JournalItemIdentity::Remote(id) => self
                .items
                .iter_mut()
                .find(|component| component.id == id.get())?
                .process_event(event)?,
            JournalItemIdentity::Local => match self.local_item.as_mut()?.process_event(event)? {
                LocalEventProcessResult::CursorLeavedFromBelow { x } => {
                    ChildEventProcessResult::CursorLeavedFromBelow { x }
                }
                LocalEventProcessResult::CursorLeavedFromAbove { x } => {
                    ChildEventProcessResult::CursorLeavedFromAbove { x }
                }
                LocalEventProcessResult::EditRequested { notes } => {
                    return Some(EventProcessResult::EditLocalJournalRequested { notes });
                }
                LocalEventProcessResult::SaveRequested => {
                    return Some(EventProcessResult::SaveLocalJournalRequested);
                }
            },
        };

        match result {
            ChildEventProcessResult::CursorLeavedFromBelow { x } => {
                let focused_index = self.item_index(focused_item)?;
                if let Some(next_item) = self.item_identity_at(focused_index + 1) {
                    self.unfocus_item(focused_item);
                    self.focused_item = Some(next_item);
                    self.focus_item(next_item, ChildFocusEvent::CursorEnteredFromAbove { x });
                    None
                } else {
                    self.unfocus_item(focused_item);
                    self.focused_item = None;
                    self.create_button_focused = true;
                    None
                }
            }
            ChildEventProcessResult::CursorLeavedFromAbove { x } => {
                let focused_index = self.item_index(focused_item)?;
                if focused_index > 0 {
                    let previous_item = self.item_identity_at(focused_index - 1)?;
                    self.unfocus_item(focused_item);
                    self.focused_item = Some(previous_item);
                    self.focus_item(previous_item, ChildFocusEvent::CursorEnteredFromBelow { x });
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
        if let Some(old_item) = self.focused_item {
            self.unfocus_item(old_item);
        }
        self.focused_item = None;
        self.create_button_focused = false;

        match event {
            FocusEvent::Focused { position } => {
                let mut line_count_sum = 0;
                let mut new_item = None;
                for index in 0..self.item_count() {
                    let line_count = self.item_line_count(index);
                    if line_count_sum + line_count > position.y {
                        new_item = self.item_identity_at(index);
                        break;
                    }
                    line_count_sum += line_count;
                }
                let Some(new_item) = new_item else {
                    self.create_button_focused = true;
                    return;
                };
                self.focused_item = Some(new_item);
                self.focus_item(
                    new_item,
                    ChildFocusEvent::Focused {
                        position: Position::new(position.x, position.y - line_count_sum),
                    },
                );
            }
            FocusEvent::Unfocused => {
                self.focused_item = None;
            }
            FocusEvent::CursorEnteredFromAbove { x } => {
                let Some(item) = self.item_identity_at(0) else {
                    self.create_button_focused = true;
                    return;
                };
                self.focused_item = Some(item);
                self.focus_item(item, ChildFocusEvent::CursorEnteredFromAbove { x });
            }
            FocusEvent::CursorEnteredFromBelow { x } => {
                self.create_button_focused = true;
                // 作成buttonのcursor位置は固定なので、遷移元の横位置は引き継がない。
                let _ = x;
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
        // Store同期でfocus対象が消えた場合も、同じ表示位置または新しい末尾から操作を継続する。
        let focused_index = self.focused_item.and_then(|item| self.item_index(item));
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
        if let Some(focused_item) = self.focused_item
            && self.item_index(focused_item).is_none()
        {
            self.unfocus_item(focused_item);
            self.focused_item = focused_index.and_then(|index| {
                self.item_identity_at(index.min(self.item_count().saturating_sub(1)))
            });
            if let Some(new_item) = self.focused_item {
                self.focus_item(
                    new_item,
                    ChildFocusEvent::Focused {
                        position: Position { x: 0, y: 0 },
                    },
                );
            } else {
                self.create_button_focused = true;
            }
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
            widgets.push(
                local_item.create_widget(self.focused_item == Some(JournalItemIdentity::Local)),
            );
        }
        JournalsListWidget::new(
            widgets,
            self.create_button_focused,
            self.local_item.is_none(),
        )
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
            + 3
    }

    pub fn get_cursor_position(&self, width: u16) -> Position {
        let Some(focused_item) = self.focused_item else {
            if self.create_button_focused {
                return Position::new(1, self.line_count(width).saturating_sub(2));
            }
            return Position::new(0, 0);
        };
        let mut line_count = 0;
        for component in self.items.iter() {
            if focused_item == JournalItemIdentity::Remote(JournalId::new(component.id)) {
                let mut position = component.get_cursor_position();
                position.y += line_count;
                return position;
            }
            line_count += component.line_count(width);
        }
        if focused_item == JournalItemIdentity::Local && self.local_item.is_some() {
            let mut position = self.local_item.as_ref().unwrap().get_cursor_position();
            position.y += line_count;
            return position;
        }
        Position::new(0, 0)
    }

    fn item_count(&self) -> usize {
        self.items.len() + usize::from(self.local_item.is_some())
    }

    fn item_identity_at(&self, index: usize) -> Option<JournalItemIdentity> {
        self.items
            .get(index)
            .map(|component| JournalItemIdentity::Remote(JournalId::new(component.id)))
            .or_else(|| {
                (index == self.items.len() && self.local_item.is_some())
                    .then_some(JournalItemIdentity::Local)
            })
    }

    fn item_index(&self, item: JournalItemIdentity) -> Option<usize> {
        match item {
            JournalItemIdentity::Remote(id) => self
                .items
                .iter()
                .position(|component| component.id == id.get()),
            JournalItemIdentity::Local => self.local_item.as_ref().map(|_| self.items.len()),
        }
    }

    fn item_line_count(&self, index: usize) -> u16 {
        self.items.get(index).map_or_else(
            || {
                self.local_item
                    .as_ref()
                    .map_or(0, |item| item.line_count(self.width))
            },
            |item| item.line_count(self.width),
        )
    }

    fn focus_item(&mut self, item: JournalItemIdentity, event: ChildFocusEvent) {
        match item {
            JournalItemIdentity::Remote(id) => {
                if let Some(component) = self
                    .items
                    .iter_mut()
                    .find(|component| component.id == id.get())
                {
                    component.focus_event(event);
                }
            }
            JournalItemIdentity::Local => {
                if let Some(component) = &mut self.local_item {
                    component.focus_event(event);
                }
            }
        }
    }

    fn unfocus_item(&mut self, item: JournalItemIdentity) {
        self.focus_item(item, ChildFocusEvent::Unfocused);
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

    fn local_entry(notes: impl Into<String>) -> LocalJournalEntry {
        LocalJournalEntry {
            journal: LocalJournal {
                issue_id: IssueId::new(1),
                notes: notes.into(),
            },
            state: LocalJournalState::LocalOnly { failure: None },
        }
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
    fn process_event_e_on_focused_local_item_returns_local_edit_requested() {
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditLocalJournalRequested { notes }) => {
                assert_eq!(notes, "local notes");
            }
            _ => panic!("expected local edit request"),
        }
    }

    #[test]
    fn process_event_enter_on_enabled_create_button_returns_create_requested() {
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], None, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        assert!(matches!(
            component.process_event(key_event(KeyCode::Enter)),
            Some(EventProcessResult::CreateLocalJournalRequested)
        ));
    }

    #[test]
    fn process_event_enter_on_disabled_create_button_is_a_no_op() {
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        assert!(component.process_event(key_event(KeyCode::Enter)).is_none());
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
    fn process_event_moves_from_last_remote_item_through_local_item_and_button() {
        let mut store = Store::new();
        let journal = create_journal(1, "remote notes");
        register_journal(&mut store, &journal);
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            Some(&local_entry),
            WIDE_WIDTH,
        );
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 3 });

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        assert_eq!(component.focused_item, Some(JournalItemIdentity::Local));
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(3, component.items[0].line_count(WIDE_WIDTH) + 3)
        );

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('j'))),
            None
        ));
        assert!(component.create_button_focused);

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
    }

    #[test]
    fn process_event_moves_from_button_through_local_item_to_last_remote_item() {
        let mut store = Store::new();
        let journal = create_journal(1, "remote notes");
        register_journal(&mut store, &journal);
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            Some(&local_entry),
            WIDE_WIDTH,
        );
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 4 });

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(component.focused_item, Some(JournalItemIdentity::Local));

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        assert_eq!(
            component.focused_item,
            Some(JournalItemIdentity::Remote(journal.id))
        );
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(0, 3)
        );
    }

    #[test]
    fn empty_list_focuses_create_button_from_above_and_below() {
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], None, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 5 });
        assert!(component.create_button_focused);
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(1, 1)
        );

        component.focus_event(FocusEvent::Unfocused);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 5 });
        assert!(component.create_button_focused);
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(1, 1)
        );
    }

    #[test]
    fn empty_list_button_leaves_above_and_below() {
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], None, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('k'))),
            Some(EventProcessResult::CursorLeavedFromAbove)
        ));
        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('j'))),
            Some(EventProcessResult::CursorLeavedFromBelow)
        ));
    }

    #[test]
    fn focus_event_unfocused_releases_local_item_focus() {
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 2 });

        component.focus_event(FocusEvent::Unfocused);

        assert_eq!(component.focused_item, None);
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(0, 0)
        );
        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
    }

    #[test]
    fn update_keeps_local_item_focus() {
        let mut local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 2 });
        local_entry.journal.notes = "updated local notes".to_string();

        component.update(&[], Some(&local_entry), WIDE_WIDTH);

        assert_eq!(component.focused_item, Some(JournalItemIdentity::Local));
        assert_eq!(
            component.get_cursor_position(WIDE_WIDTH),
            Position::new(2, 3)
        );
    }

    #[test]
    fn snapshot_focused_local_item() {
        let store = Store::new();
        let local_entry = local_entry("focused local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        let widget = component.create_widget(&store);
        let line_count = widget.line_count(WIDE_WIDTH);

        render_snapshot(
            "journals_list_local_item_focused",
            WIDE_WIDTH,
            line_count,
            widget,
        );
    }

    #[test]
    fn process_event_ctrl_s_on_local_only_item_returns_save_local_journal_requested() {
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(ctrl_s_event());

        assert!(matches!(
            result,
            Some(EventProcessResult::SaveLocalJournalRequested)
        ));
    }

    #[test]
    fn process_event_ctrl_s_on_uploading_local_item_is_a_no_op() {
        let local_entry = LocalJournalEntry {
            journal: LocalJournal {
                issue_id: IssueId::new(1),
                notes: "local notes".to_string(),
            },
            state: LocalJournalState::Uploading,
        };
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        assert!(component.process_event(ctrl_s_event()).is_none());
    }

    fn uploading_local_component(
        store: &mut Store,
        remote_journals: &[Journal],
    ) -> JournalsListComponent {
        let issue_id = IssueId::new(1);
        if !remote_journals.is_empty() {
            store.consume_action(Action::Journal(JournalAction::SyncFetched {
                issue_id,
                journals: remote_journals.to_vec(),
            }));
        }
        store.consume_action(Action::Journal(JournalAction::CreateLocal { issue_id }));
        store.consume_action(Action::Journal(JournalAction::EditLocalNotes {
            issue_id,
            notes: "local notes".to_string(),
        }));
        store.consume_action(Action::Journal(JournalAction::StartLocalUpload {
            issue_id,
        }));
        let mut component = JournalsListComponent::new(issue_id);
        component.update(
            store.get_remote_journals(issue_id),
            store.get_local_journal(issue_id),
            WIDE_WIDTH,
        );
        component
    }

    fn complete_local_upload(store: &mut Store, journals: Vec<Journal>) {
        store.consume_action(Action::Journal(
            JournalAction::CompleteLocalUploadWithFetched {
                issue_id: IssueId::new(1),
                journals,
            },
        ));
    }

    #[test]
    fn update_after_local_upload_completion_keeps_the_focus_index_when_more_items_follow() {
        let mut store = Store::new();
        let remotes = vec![create_journal(1, "first"), create_journal(2, "second")];
        let mut component = uploading_local_component(&mut store, &remotes);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        component.process_event(key_event(KeyCode::Char('j')));
        assert_eq!(
            component.focused_item,
            Some(JournalItemIdentity::Remote(JournalId::new(2)))
        );

        let mut fetched = remotes.clone();
        fetched.push(create_journal(3, "local notes"));
        complete_local_upload(&mut store, fetched);
        component.update(
            store.get_remote_journals(IssueId::new(1)),
            store.get_local_journal(IssueId::new(1)),
            WIDE_WIDTH,
        );

        assert_eq!(
            component.focused_item,
            Some(JournalItemIdentity::Remote(JournalId::new(2)))
        );
        assert!(!component.create_button_focused);
    }

    #[test]
    fn update_after_local_upload_completion_clamps_the_focus_index_to_the_last_item() {
        let mut store = Store::new();
        let remotes = vec![create_journal(1, "first")];
        let mut component = uploading_local_component(&mut store, &remotes);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });
        component.process_event(key_event(KeyCode::Char('k')));
        assert_eq!(component.focused_item, Some(JournalItemIdentity::Local));

        complete_local_upload(&mut store, remotes.clone());
        component.update(
            store.get_remote_journals(IssueId::new(1)),
            store.get_local_journal(IssueId::new(1)),
            WIDE_WIDTH,
        );

        assert_eq!(
            component.focused_item,
            Some(JournalItemIdentity::Remote(JournalId::new(1)))
        );
        assert!(!component.create_button_focused);
    }

    #[test]
    fn update_after_local_upload_completion_moves_focus_to_the_create_button_when_the_list_is_empty()
     {
        let mut store = Store::new();
        let mut component = uploading_local_component(&mut store, &[]);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });
        assert_eq!(component.focused_item, Some(JournalItemIdentity::Local));

        complete_local_upload(&mut store, vec![]);
        component.update(
            store.get_remote_journals(IssueId::new(1)),
            store.get_local_journal(IssueId::new(1)),
            WIDE_WIDTH,
        );

        assert_eq!(component.focused_item, None);
        assert!(component.create_button_focused);
    }

    #[test]
    fn update_moves_focus_to_last_remote_item_when_local_item_disappears() {
        let mut store = Store::new();
        let journal = create_journal(1, "remote notes");
        register_journal(&mut store, &journal);
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            Some(&local_entry),
            WIDE_WIDTH,
        );
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });
        component.process_event(key_event(KeyCode::Char('k')));

        component.update(
            store.get_remote_journals(journal.issue_id),
            None,
            WIDE_WIDTH,
        );

        assert_eq!(
            component.focused_item,
            Some(JournalItemIdentity::Remote(journal.id))
        );
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

    #[test]
    fn snapshot_empty_list_with_focused_create_button() {
        let store = Store::new();
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], None, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        render_snapshot(
            "journals_list_empty_with_focused_create_button",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn snapshot_local_item_disables_focused_create_button() {
        let store = Store::new();
        let local_entry = local_entry("local notes");
        let mut component = JournalsListComponent::new(IssueId::new(1));
        component.update(&[], Some(&local_entry), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        render_snapshot(
            "journals_list_local_item_disables_focused_create_button",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn snapshot_list_clips_create_button() {
        let mut store = Store::new();
        let journal = create_journal(1, "remote notes");
        register_journal(&mut store, &journal);
        let mut component = JournalsListComponent::new(journal.issue_id);
        component.update(
            store.get_remote_journals(journal.issue_id),
            None,
            WIDE_WIDTH,
        );
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });
        let height = component.line_count(WIDE_WIDTH) - 1;

        render_snapshot(
            "journals_list_clipped_create_button",
            WIDE_WIDTH,
            height,
            component.create_widget(&store),
        );
    }
}
