use crossterm::event::Event;
use ratatui::layout::Position;

use crate::entities::{Journal, LocalJournal};
use crate::stores::Store;
use crate::vos::{EntityIdValue, JournalDetail, JournalDetailAttr, JournalKey};

use super::focus_state;
pub use super::focus_state::FocusEvent;
use super::focus_state::FocusState;
use super::widget::ResolvedJournalDetail;
use super::{JournalItemDisplay, JournalItemWidget, JournalItemWidgetState};

/// Listからitem updateへ渡す、key種別とentry種別が一致する表示content。
pub enum JournalItemContent<'a> {
    Remote(&'a Journal),
    Local(&'a LocalJournal),
}

enum JournalData {
    Remote(Journal),
    Local(LocalJournal),
}

impl JournalData {
    fn from_content(key: JournalKey, content: &JournalItemContent) -> Self {
        match (key, content) {
            (JournalKey::Remote(_), JournalItemContent::Remote(journal)) => {
                Self::Remote((*journal).clone())
            }
            (JournalKey::Local(_), JournalItemContent::Local(journal)) => {
                Self::Local((*journal).clone())
            }
            _ => panic!("journal key and content kind must match (Store invariant violated)"),
        }
    }

    fn notes(&self) -> &str {
        match self {
            Self::Remote(journal) => &journal.notes,
            Self::Local(journal) => &journal.notes,
        }
    }

    fn detail_count(&self) -> usize {
        match self {
            Self::Remote(journal) => journal.details.len(),
            Self::Local(_) => 0,
        }
    }
}

const NONE_DISPLAY: &str = "(なし)";
const UNKNOWN_DISPLAY: &str = "(不明)";

fn resolve_journal_details(details: &[JournalDetail], store: &Store) -> Vec<ResolvedJournalDetail> {
    details
        .iter()
        .map(|JournalDetail::Attr(attr)| resolve_attr(attr, store))
        .collect()
}

fn resolve_attr(attr: &JournalDetailAttr, store: &Store) -> ResolvedJournalDetail {
    let (field_label, old_display, new_display) = match attr {
        JournalDetailAttr::StatusId { old, new } => (
            "ステータス",
            store.get_issue_status(*old).name.clone(),
            store.get_issue_status(*new).name.clone(),
        ),
        JournalDetailAttr::TrackerId { old, new } => (
            "トラッカー",
            store
                .get_tracker(*old)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
            store
                .get_tracker(*new)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
        ),
        JournalDetailAttr::ProjectId { old, new } => (
            "プロジェクト",
            store
                .get_project(*old)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
            store
                .get_project(*new)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
        ),
        JournalDetailAttr::Subject { old, new } => ("件名", old.clone(), new.clone()),
        JournalDetailAttr::Description { .. } => ("説明", "(変更あり)".into(), "(変更あり)".into()),
        JournalDetailAttr::CategoryId { old, new } => (
            "カテゴリ",
            resolve_optional(*old, |id| store.get_category(id).map(|v| v.name.clone())),
            resolve_optional(*new, |id| store.get_category(id).map(|v| v.name.clone())),
        ),
        JournalDetailAttr::AssignedToId { old, new } => (
            "担当者",
            resolve_optional(*old, |id| store.get_user(id).map(|v| v.name.clone())),
            resolve_optional(*new, |id| store.get_user(id).map(|v| v.name.clone())),
        ),
        JournalDetailAttr::PriorityId { old, new } => (
            "優先度",
            store
                .get_priority(*old)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
            store
                .get_priority(*new)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
        ),
        JournalDetailAttr::FixedVersionId { old, new } => (
            "対象バージョン",
            resolve_optional(*old, |id| {
                store.get_target_version(id).map(|v| v.name.clone())
            }),
            resolve_optional(*new, |id| {
                store.get_target_version(id).map(|v| v.name.clone())
            }),
        ),
        JournalDetailAttr::AuthorId { old, new } => (
            "作成者",
            store
                .get_user(*old)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
            store
                .get_user(*new)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
        ),
        JournalDetailAttr::StartDate { old, new } => {
            ("開始日", format_date(*old), format_date(*new))
        }
        JournalDetailAttr::DueDate { old, new } => ("期日", format_date(*old), format_date(*new)),
        JournalDetailAttr::DoneRatio { old, new } => {
            ("進捗率", format!("{}%", old), format!("{}%", new))
        }
        JournalDetailAttr::EstimatedHours { old, new } => (
            "予定工数",
            old.map_or(NONE_DISPLAY.into(), |v| format!("{}h", v)),
            new.map_or(NONE_DISPLAY.into(), |v| format!("{}h", v)),
        ),
        JournalDetailAttr::ParentId { old, new } => (
            "親チケット",
            resolve_optional(*old, |id| Some(resolve_parent_label(id, store))),
            resolve_optional(*new, |id| Some(resolve_parent_label(id, store))),
        ),
        JournalDetailAttr::IsPrivate { old, new } => {
            ("非公開", format_bool(*old), format_bool(*new))
        }
    };
    ResolvedJournalDetail {
        field_label,
        old_display,
        new_display,
    }
}

fn resolve_optional<Id: EntityIdValue>(
    id: Option<Id>,
    lookup: impl FnOnce(Id) -> Option<String>,
) -> String {
    id.and_then(lookup).unwrap_or(NONE_DISPLAY.into())
}

fn resolve_parent_label(id: crate::vos::IssueId, store: &Store) -> String {
    match store.get_issue(id) {
        Some((issue, _)) => format!("#{} {}", id, issue.issue.subject),
        None => format!("#{}", id),
    }
}

fn format_date(date: Option<chrono::DateTime<chrono::Local>>) -> String {
    date.map_or(NONE_DISPLAY.into(), |d| d.format("%Y/%m/%d").to_string())
}

fn format_bool(b: bool) -> String {
    if b { "はい" } else { "いいえ" }.to_string()
}

pub enum EventProcessResult {
    CursorLeavedFromBelow { x: u16 },
    CursorLeavedFromAbove { x: u16 },
    EditRequested { key: JournalKey, notes: String },
}

pub struct JournalsListItemComponent {
    pub key: JournalKey,
    data: JournalData,
    comment_line_count: u16,
    focus_state: FocusState,
    widget_state: JournalItemWidgetState,
}

impl JournalsListItemComponent {
    pub fn new(key: JournalKey, content: &JournalItemContent) -> Self {
        Self {
            data: JournalData::from_content(key, content),
            key,
            comment_line_count: 0,
            focus_state: FocusState::new(),
            widget_state: JournalItemWidgetState::new(),
        }
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .map(|result| match result {
                focus_state::EventProcessResult::CursorLeavedFromBelow { x } => {
                    EventProcessResult::CursorLeavedFromBelow { x }
                }
                focus_state::EventProcessResult::CursorLeavedFromAbove { x } => {
                    EventProcessResult::CursorLeavedFromAbove { x }
                }
                focus_state::EventProcessResult::Edit => EventProcessResult::EditRequested {
                    key: self.key,
                    notes: self.data.notes().to_string(),
                },
            })
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, content: &JournalItemContent, width: u16) {
        let data = JournalData::from_content(self.key, content);
        self.widget_state.update(width, data.notes());
        self.comment_line_count = self.widget_state.comment_line_count();
        self.focus_state
            .update(width, data.detail_count(), self.comment_line_count);
        self.data = data;
    }

    pub fn create_widget<'a>(&'a self, store: &Store) -> JournalItemWidget<'a> {
        let display = match &self.data {
            JournalData::Remote(journal) => JournalItemDisplay::Remote {
                user: &journal.user,
                updated_on: &journal.updated_on,
                details: resolve_journal_details(&journal.details, store),
            },
            JournalData::Local(_) => JournalItemDisplay::Local,
        };
        JournalItemWidget::new(display, &self.widget_state, self.focus_state.is_focused())
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.data.detail_count() as u16 + 1 + self.comment_line_count
    }

    pub fn get_cursor_position(&self) -> Position {
        self.focus_state.get_cursor_position()
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        entities::{Journal, LocalJournal},
        stores::Store,
        test_support::{local_datetime, render_snapshot, sync_fixture_entities},
        vos::{
            IssueStatusId, JournalDetail, JournalDetailAttr, JournalId, JournalKey, LocalJournalId,
            UserId,
        },
    };

    const WIDE_WIDTH: u16 = 32;
    const NARROW_WIDTH: u16 = 18;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn fixture_store() -> Store {
        let mut store = Store::new();
        sync_fixture_entities(&mut store);
        store
    }

    fn create_journal(id: u16, details: Vec<JournalDetail>, notes: impl Into<String>) -> Journal {
        Journal {
            id: JournalId::new(id),
            user: "alice".to_string(),
            updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
            details,
            notes: notes.into(),
        }
    }

    fn assigned_to_detail(old: Option<u16>, new: Option<u16>) -> JournalDetail {
        JournalDetail::Attr(JournalDetailAttr::AssignedToId {
            old: old.map(UserId::new),
            new: new.map(UserId::new),
        })
    }

    fn status_detail(old: u16, new: u16) -> JournalDetail {
        JournalDetail::Attr(JournalDetailAttr::StatusId {
            old: IssueStatusId::new(old),
            new: IssueStatusId::new(new),
        })
    }

    fn details() -> Vec<JournalDetail> {
        vec![status_detail(1, 2), assigned_to_detail(None, Some(1001))]
    }

    fn one_detail() -> Vec<JournalDetail> {
        vec![assigned_to_detail(None, Some(1001))]
    }

    fn notes() -> &'static str {
        "first paragraph\n\nsecond paragraph with wrapping words"
    }

    fn updated_notes() -> &'static str {
        "updated notes with enough text to wrap onto a different set of rendered lines"
    }

    fn component_with_update(journal: &Journal, width: u16) -> JournalsListItemComponent {
        let content = JournalItemContent::Remote(journal);
        let mut component =
            JournalsListItemComponent::new(JournalKey::Remote(journal.id), &content);
        component.update(&content, width);
        component
    }

    fn create_local_journal(id: u64, notes: impl Into<String>) -> LocalJournal {
        LocalJournal {
            id: LocalJournalId::new(id),
            issue_id: 3.into(),
            notes: notes.into(),
        }
    }

    fn local_component_with_update(local: &LocalJournal, width: u16) -> JournalsListItemComponent {
        let content = JournalItemContent::Local(local);
        let mut component = JournalsListItemComponent::new(JournalKey::Local(local.id), &content);
        component.update(&content, width);
        component
    }

    fn assert_layout_contract(
        component: &JournalsListItemComponent,
        width: u16,
        line_count: u16,
        cursor: Position,
    ) {
        assert_eq!(component.line_count(width), line_count);
        assert_eq!(component.get_cursor_position(), cursor);
    }

    #[test]
    fn update_initial_state_is_unfocused_and_rendered() {
        let store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let content = JournalItemContent::Remote(&journal);
        let mut component =
            JournalsListItemComponent::new(JournalKey::Remote(journal.id), &content);

        component.update(&content, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_initial_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn update_changed_notes_updates_line_count_and_widget() {
        let store = fixture_store();
        let initial_journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        let updated_journal = create_journal(1, one_detail(), updated_notes());
        let updated_content = JournalItemContent::Remote(&updated_journal);

        component.update(&updated_content, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 7, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_update_changed_notes",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn focus_event_updates_cursor_and_widget_focus() {
        let store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let content = JournalItemContent::Remote(&journal);
        let mut component =
            JournalsListItemComponent::new(JournalKey::Remote(journal.id), &content);
        component.update(&content, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 2));
        render_snapshot(
            "journals_list_item_component_focus_from_above_to_detail",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn unfocused_removes_widget_focus() {
        let store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        component.focus_event(FocusEvent::Unfocused);

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_delegates_to_focus_state() {
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(0, 3));
    }

    #[test]
    fn process_event_e_on_notes_position_returns_edit_requested_with_current_notes() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested {
                key,
                notes: edit_notes,
            }) => {
                assert_eq!(key, JournalKey::Remote(JournalId::new(1)));
                assert_eq!(edit_notes, notes());
            }
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    fn update_passes_latest_width_and_comment_line_count_to_focus_state() {
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(31, 6),
        });

        let content = JournalItemContent::Remote(&journal);
        component.update(&content, NARROW_WIDTH);

        assert_layout_contract(&component, NARROW_WIDTH, 9, Position::new(17, 6));
    }

    #[test]
    fn update_passes_latest_property_count_to_focus_state() {
        let initial_journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&initial_journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(0, 3),
        });
        let updated_journal = create_journal(1, one_detail(), notes());
        let updated_content = JournalItemContent::Remote(&updated_journal);

        component.update(&updated_content, WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 8, Position::new(0, 2));
    }

    #[test]
    fn local_item_process_event_e_on_notes_returns_edit_requested_with_local_key_and_notes() {
        let local = create_local_journal(1, notes());
        let mut component = local_component_with_update(&local, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested {
                key,
                notes: edit_notes,
            }) => {
                assert_eq!(key, JournalKey::Local(LocalJournalId::new(1)));
                assert_eq!(edit_notes, notes());
            }
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    fn local_item_line_count_has_no_property_details_lines() {
        let local = create_local_journal(1, notes());
        let component = local_component_with_update(&local, WIDE_WIDTH);

        // title + 空行2 + Notes(4行)でdetail行は持たない
        assert_layout_contract(&component, WIDE_WIDTH, 7, Position::new(0, 0));
    }

    #[test]
    fn snapshot_local_journal_item_initial_unfocused() {
        let store = fixture_store();
        let local = create_local_journal(1, notes());
        let component = local_component_with_update(&local, WIDE_WIDTH);

        render_snapshot(
            "journals_list_item_component_local_journal_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn snapshot_local_journal_item_focused() {
        let store = fixture_store();
        let local = create_local_journal(1, notes());
        let mut component = local_component_with_update(&local, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        assert_layout_contract(&component, WIDE_WIDTH, 7, Position::new(0, 6));
        render_snapshot(
            "journals_list_item_component_local_journal_focused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    #[should_panic(expected = "journal key and content kind must match")]
    fn new_panics_when_key_kind_mismatches_content() {
        let local = create_local_journal(1, notes());
        let content = JournalItemContent::Local(&local);
        let _ = JournalsListItemComponent::new(JournalKey::Remote(JournalId::new(1)), &content);
    }

    #[test]
    #[should_panic(expected = "journal key and content kind must match")]
    fn update_panics_when_key_kind_mismatches_content() {
        let local = create_local_journal(1, notes());
        let mut component = local_component_with_update(&local, WIDE_WIDTH);
        let content = JournalItemContent::Local(&local);

        component.key = JournalKey::Remote(JournalId::new(1));
        component.update(&content, WIDE_WIDTH);
    }
}
