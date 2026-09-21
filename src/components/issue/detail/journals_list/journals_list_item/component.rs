use crossterm::event::Event;
use ratatui::layout::Position;

use crate::inputs::native::convert_key;
use crate::stores::{RemoteJournalEntry, RemoteJournalState, Store};
use crate::vos::{EntityIdValue, IssueId, JournalDetail, JournalDetailAttr, JournalId};

use super::focus_state;
pub use super::focus_state::FocusEvent;
use super::focus_state::FocusState;
use super::widget::RemoteJournalItemView;
use super::widget::{ResolvedJournalDetail, display_notes, state_marker};
use super::{JournalItemWidget, JournalItemWidgetState};

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
            store
                .get_issue_status(*old)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
            store
                .get_issue_status(*new)
                .map_or(UNKNOWN_DISPLAY.into(), |v| v.name.clone()),
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
    if store.try_get_issue_state(id).is_none() {
        return format!("#{}", id);
    }
    let (issue, _) = store.get_issue(id);
    format!("#{} {}", id, issue.issue.subject)
}

fn format_date(date: Option<chrono::DateTime<chrono::Local>>) -> String {
    date.map_or(NONE_DISPLAY.into(), |d| d.format("%Y/%m/%d").to_string())
}

fn format_bool(b: bool) -> String {
    if b { "はい" } else { "いいえ" }.to_string()
}

pub enum EventProcessResult {
    CursorLeavedFromBelow {
        x: u16,
    },
    CursorLeavedFromAbove {
        x: u16,
    },
    EditRequested {
        id: JournalId,
        notes: String,
    },
    SaveRequested {
        id: JournalId,
    },
    /// 保存キーを正常なno-opとして消費済みであり、未処理を表す`None`とは区別する。
    SaveSuppressed,
}

pub struct JournalsListItemComponent {
    pub id: u16,
    issue_id: IssueId,
    notes: String,
    state: RemoteJournalState,
    detail_count: usize,
    comment_line_count: u16,
    focus_state: FocusState,
    widget_state: JournalItemWidgetState,
}

impl JournalsListItemComponent {
    pub fn new(issue_id: IssueId, journal_id: JournalId) -> Self {
        Self {
            id: journal_id.get(),
            issue_id,
            notes: String::new(),
            state: RemoteJournalState::Synced,
            detail_count: 0,
            comment_line_count: 0,
            focus_state: FocusState::new(),
            widget_state: JournalItemWidgetState::new(),
        }
    }

    /// 保存対象がない状態、または保存処理中の重複入力として保存キーを消費するかを返す。
    fn save_key_is_no_op(&self) -> bool {
        matches!(
            self.state,
            RemoteJournalState::Synced | RemoteJournalState::Uploading { .. }
        )
    }

    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        let Event::Key(key) = event else {
            return None;
        };
        // 上位Componentの入力契約がcrosstermの間だけ、共通入力へ移行済みのFocusStateとの境界で変換する。
        let event = convert_key(key)?;
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
                    id: JournalId::new(self.id),
                    notes: self.notes.clone(),
                },
                focus_state::EventProcessResult::SaveRequested => {
                    EventProcessResult::SaveRequested {
                        id: JournalId::new(self.id),
                    }
                }
                focus_state::EventProcessResult::SaveSuppressed => {
                    EventProcessResult::SaveSuppressed
                }
            })
    }

    pub fn focus_event(&mut self, event: FocusEvent) {
        self.focus_state.focus_event(event);
    }

    pub fn update(&mut self, entry: &RemoteJournalEntry, width: u16) {
        let journal = &entry.journal;
        let notes = display_notes(&journal.notes, &entry.state).to_owned();
        self.notes = notes;
        self.state = entry.state.clone();
        self.detail_count = journal.details.len();
        self.widget_state.update(
            width,
            &journal.user,
            journal.updated_on.as_ref(),
            &self.notes,
        );
        self.comment_line_count = self.widget_state.comment_line_count();
        self.focus_state.update(
            width,
            self.detail_count,
            self.comment_line_count,
            self.save_key_is_no_op(),
        );
    }

    pub fn create_widget<'a>(&'a self, store: &'a Store) -> JournalItemWidget<'a> {
        let entry = store.get_remote_journal(self.issue_id, JournalId::new(self.id));
        let view = RemoteJournalItemView {
            user: &entry.journal.user,
            updated_on: entry.journal.updated_on.as_ref(),
            notes: &self.notes,
            state_marker: state_marker(&entry.state),
        };
        JournalItemWidget::new(
            view,
            resolve_journal_details(&entry.journal.details, store),
            &self.widget_state,
            self.focus_state.is_focused(),
        )
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.detail_count as u16 + 1 + self.comment_line_count + 1
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
        entities::Journal,
        stores::{Action, JournalAction, RemoteJournalEntry, Store},
        test_support::{local_datetime, render_snapshot, sync_fixture_entities},
        vos::{IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId, UserId},
    };

    const WIDE_WIDTH: u16 = 32;
    const NARROW_WIDTH: u16 = 18;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl_s_event() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
    }

    fn make_edited(store: &mut Store, journal: &Journal) -> RemoteJournalEntry {
        store.consume_action(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: journal.issue_id,
            journal_id: journal.id,
            notes: "edited notes".to_string(),
        }));
        store
            .get_remote_journal(journal.issue_id, journal.id)
            .clone()
    }

    fn make_uploading(store: &mut Store, journal: &Journal) -> RemoteJournalEntry {
        store.consume_action(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: journal.issue_id,
            journal_id: journal.id,
            notes: "edited notes".to_string(),
        }));
        store.consume_action(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: journal.issue_id,
            journal_id: journal.id,
        }));
        store
            .get_remote_journal(journal.issue_id, journal.id)
            .clone()
    }

    fn fixture_store() -> Store {
        let mut store = Store::new();
        sync_fixture_entities(&mut store);
        store
    }

    fn create_journal(id: u16, details: Vec<JournalDetail>, notes: impl Into<String>) -> Journal {
        Journal {
            id: JournalId::new(id),
            issue_id: IssueId::new(1),
            user: "alice".to_string(),
            updated_on: Some(local_datetime("2026-01-15T00:00:00+09:00")),
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

    fn register_journal<'s>(store: &'s mut Store, journal: &Journal) -> &'s RemoteJournalEntry {
        store.consume_action(Action::Journal(JournalAction::SyncFetched {
            issue_id: journal.issue_id,
            journals: vec![journal.clone()],
        }));
        store.get_remote_journal(journal.issue_id, journal.id)
    }

    fn component_with_update(
        store: &mut Store,
        journal: &Journal,
        width: u16,
    ) -> JournalsListItemComponent {
        let mut component = JournalsListItemComponent::new(journal.issue_id, journal.id);
        let entry = register_journal(store, journal);
        component.update(entry, width);
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
        let mut store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let mut component = JournalsListItemComponent::new(journal.issue_id, journal.id);

        component.update(register_journal(&mut store, &journal), WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 10, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_initial_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn snapshot_state_marker_edited_after_edit_remote_notes() {
        let mut store = fixture_store();
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        store.consume_action(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: journal.issue_id,
            journal_id: journal.id,
            notes: "edited notes for the marker snapshot".to_string(),
        }));

        component.update(
            store.get_remote_journal(journal.issue_id, journal.id),
            WIDE_WIDTH,
        );

        render_snapshot(
            "journals_list_item_component_state_marker_edited",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn update_changed_notes_updates_line_count_and_widget() {
        let mut store = fixture_store();
        let initial_journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &initial_journal, WIDE_WIDTH);
        let updated_journal = create_journal(1, one_detail(), updated_notes());

        component.update(register_journal(&mut store, &updated_journal), WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 8, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_update_changed_notes",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn focus_event_updates_cursor_and_widget_focus() {
        let mut store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);

        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        assert_layout_contract(&component, WIDE_WIDTH, 10, Position::new(2, 2));
        render_snapshot(
            "journals_list_item_component_focus_from_above_to_detail",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn unfocused_removes_widget_focus() {
        let mut store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 6 });

        component.focus_event(FocusEvent::Unfocused);

        assert_layout_contract(&component, WIDE_WIDTH, 10, Position::new(0, 0));
        render_snapshot(
            "journals_list_item_component_unfocused",
            WIDE_WIDTH,
            component.line_count(WIDE_WIDTH),
            component.create_widget(&store),
        );
    }

    #[test]
    fn process_event_delegates_to_focus_state() {
        let mut store = fixture_store();
        let journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromAbove { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('j')));

        assert!(result.is_none());
        assert_layout_contract(&component, WIDE_WIDTH, 10, Position::new(2, 3));
    }

    #[test]
    fn process_event_e_on_notes_position_returns_edit_requested_with_current_notes() {
        let mut store = fixture_store();
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(key_event(KeyCode::Char('e')));

        match result {
            Some(EventProcessResult::EditRequested {
                id,
                notes: edit_notes,
            }) => {
                assert_eq!(id, JournalId::new(1));
                assert_eq!(edit_notes, notes());
            }
            _ => panic!("expected edit request"),
        }
    }

    #[test]
    fn update_passes_latest_width_and_comment_line_count_to_focus_state() {
        let mut store = fixture_store();
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(31, 6),
        });

        component.update(
            store.get_remote_journal(journal.issue_id, journal.id),
            NARROW_WIDTH,
        );

        assert_layout_contract(&component, NARROW_WIDTH, 10, Position::new(19, 6));
    }

    #[test]
    fn process_event_ctrl_s_on_edited_returns_save_requested() {
        let mut store = fixture_store();
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.update(&make_edited(&mut store, &journal), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(ctrl_s_event());

        match result {
            Some(EventProcessResult::SaveRequested { id }) => {
                assert_eq!(id, JournalId::new(1));
            }
            _ => panic!("expected save request"),
        }
    }

    #[test]
    fn process_event_ctrl_s_on_synced_returns_save_suppressed() {
        let mut store = fixture_store();
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(ctrl_s_event());

        assert!(matches!(result, Some(EventProcessResult::SaveSuppressed)));
    }

    #[test]
    fn process_event_ctrl_s_on_uploading_returns_save_suppressed() {
        let mut store = fixture_store();
        let journal = create_journal(1, one_detail(), notes());
        let mut component = component_with_update(&mut store, &journal, WIDE_WIDTH);
        component.update(&make_uploading(&mut store, &journal), WIDE_WIDTH);
        component.focus_event(FocusEvent::CursorEnteredFromBelow { x: 0 });

        let result = component.process_event(ctrl_s_event());

        assert!(matches!(result, Some(EventProcessResult::SaveSuppressed)));
    }

    #[test]
    fn resolve_journal_details_status_falls_back_to_unknown_for_missing_ids() {
        let mut store = fixture_store();
        let journal = create_journal(1, vec![status_detail(1, 99), status_detail(99, 2)], notes());
        register_journal(&mut store, &journal);

        let details = resolve_journal_details(&journal.details, &store);

        assert_eq!(details[0].field_label, "ステータス");
        assert_eq!(
            details[0].old_display,
            store
                .get_issue_status(IssueStatusId::new(1))
                .unwrap()
                .name
                .clone()
        );
        assert_eq!(details[0].new_display, UNKNOWN_DISPLAY);

        assert_eq!(details[1].field_label, "ステータス");
        assert_eq!(details[1].old_display, UNKNOWN_DISPLAY);
        assert_eq!(
            details[1].new_display,
            store
                .get_issue_status(IssueStatusId::new(2))
                .unwrap()
                .name
                .clone()
        );
    }

    #[test]
    fn update_passes_latest_property_count_to_focus_state() {
        let mut store = fixture_store();
        let initial_journal = create_journal(1, details(), notes());
        let mut component = component_with_update(&mut store, &initial_journal, WIDE_WIDTH);
        component.focus_event(FocusEvent::Focused {
            position: Position::new(0, 3),
        });
        let updated_journal = create_journal(1, one_detail(), notes());

        component.update(register_journal(&mut store, &updated_journal), WIDE_WIDTH);

        assert_layout_contract(&component, WIDE_WIDTH, 9, Position::new(2, 2));
    }
}
