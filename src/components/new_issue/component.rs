use std::cell::RefCell;
use std::rc::Rc;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::platform::input::{InputEvent, KeyCode};
use crate::stores::Store;
use crate::usecases::issue_popup_options::{
    build_assigned_to_options, build_category_options, build_done_ratio_options,
    build_issue_status_options, build_priority_options, build_target_version_options,
    build_tracker_options,
};
use crate::usecases::{IssueCreationId, NewIssueDraft};
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

use super::NewIssueWidget;

/// 下書きで編集できる項目。表示順に並べる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftField {
    Subject,
    Description,
    Tracker,
    Status,
    Priority,
    AssignedTo,
    TargetVersion,
    StartDate,
    DueDate,
    DoneRatio,
    EstimatedHours,
    Category,
}

const FIELDS: [DraftField; 12] = [
    DraftField::Subject,
    DraftField::Description,
    DraftField::Tracker,
    DraftField::Status,
    DraftField::Priority,
    DraftField::AssignedTo,
    DraftField::TargetVersion,
    DraftField::StartDate,
    DraftField::DueDate,
    DraftField::DoneRatio,
    DraftField::EstimatedHours,
    DraftField::Category,
];

#[derive(Debug, PartialEq, Eq)]
pub enum EventProcessResult {
    EditFieldRequested { field: DraftField },
    OpenIssueSelectPopup,
    Handled,
}

/// select boxで値を選ぶ項目の、選択肢・フォーカス位置・確定値を下書きへ書き戻すobserver。
pub struct DraftSelectBox {
    pub items: Vec<(u32, String)>,
    pub focused_index: usize,
    pub observer: Box<dyn FnMut(Option<u32>)>,
}

/// Issueの新規作成画面。下書きを持ち、作成の要求とその結果を`creation_id`で対応付ける。
pub struct NewIssueComponent {
    creation_id: IssueCreationId,
    /// 編集popupのobserverが書き換えるため共有する。
    draft: Rc<RefCell<NewIssueDraft>>,
    /// `FIELDS`の後に添付ファイルの追加行を数えたindex。
    focused_index: usize,
}

impl NewIssueComponent {
    pub fn new(project_id: ProjectId, parent_issue_id: Option<IssueId>) -> Self {
        Self {
            creation_id: IssueCreationId::new(),
            draft: Rc::new(RefCell::new(NewIssueDraft::new(
                project_id,
                parent_issue_id,
            ))),
            focused_index: 0,
        }
    }

    pub fn creation_id(&self) -> IssueCreationId {
        self.creation_id
    }

    pub fn draft(&self) -> Rc<RefCell<NewIssueDraft>> {
        self.draft.clone()
    }

    pub fn process_event(&mut self, event: &InputEvent) -> Option<EventProcessResult> {
        let InputEvent::Key(key) = event;
        match key.code {
            KeyCode::Char('j') => {
                self.focused_index = (self.focused_index + 1).min(FIELDS.len());
            }
            KeyCode::Char('k') => {
                self.focused_index = self.focused_index.saturating_sub(1);
            }
            KeyCode::Char('y') => return Some(EventProcessResult::OpenIssueSelectPopup),
            KeyCode::Enter => {
                if let Some(field) = FIELDS.get(self.focused_index) {
                    return Some(EventProcessResult::EditFieldRequested { field: *field });
                }
            }
            _ => return None,
        }
        Some(EventProcessResult::Handled)
    }

    /// `field`がselect boxで値を選ぶ項目なら、その選択肢を返す。
    ///
    /// 「選択なし」は値の解除か、Redmineの既定値に戻すことを表す。
    pub fn select_box(&self, store: &Store, field: DraftField) -> Option<DraftSelectBox> {
        let draft = self.draft.borrow();
        let project_id = draft.project_id;
        let ((items, focused_index), apply): (_, fn(&mut NewIssueDraft, Option<u32>)) = match field
        {
            DraftField::Tracker => (
                build_tracker_options(store, draft.tracker_id),
                |draft, id| draft.tracker_id = id.map(TrackerId::new),
            ),
            DraftField::Status => (build_issue_status_options(store), |draft, id| {
                draft.status_id = id.map(IssueStatusId::new)
            }),
            DraftField::Priority => (
                build_priority_options(store, draft.priority_id),
                |draft, id| draft.priority_id = id.map(PriorityId::new),
            ),
            DraftField::AssignedTo => (
                build_assigned_to_options(store, draft.assigned_to_id),
                |draft, id| draft.assigned_to_id = id.map(UserId::new),
            ),
            DraftField::TargetVersion => (
                build_target_version_options(store, project_id, draft.target_version_id),
                |draft, id| draft.target_version_id = id.map(TargetVersionId::new),
            ),
            DraftField::DoneRatio => (build_done_ratio_options(draft.done_ratio), |draft, id| {
                draft.done_ratio = id.map(|ratio| ratio as u16)
            }),
            DraftField::Category => (
                build_category_options(store, project_id, draft.category_id),
                |draft, id| draft.category_id = id.map(CategoryId::new),
            ),
            _ => return None,
        };
        let shared = self.draft.clone();
        Some(DraftSelectBox {
            items,
            focused_index,
            observer: Box::new(move |id| apply(&mut shared.borrow_mut(), id)),
        })
    }

    pub fn create_widget<'a>(&self, store: &'a Store) -> NewIssueWidget<'a> {
        let draft = self.draft.borrow();
        let parent = draft.parent_issue_id.map(|id| {
            let subject = store
                .try_get_issue_state(id)
                .map(|_| store.get_issue(id).0.subject())
                .unwrap_or("");
            (id, subject)
        });
        NewIssueWidget {
            project: store
                .get_project(draft.project_id)
                .map(|project| project.name.as_str())
                .unwrap_or("(unknown)"),
            parent,
            fields: FIELDS
                .iter()
                .map(|field| (label(*field), display_value(*field, &draft, store)))
                .collect(),
            attachments: Vec::new(),
            focused_index: self.focused_index,
        }
    }

    pub fn render(&self, store: &Store, frame: &mut Frame, area: Rect) {
        self.create_widget(store).render(area, frame.buffer_mut());
    }
}

fn label(field: DraftField) -> &'static str {
    match field {
        DraftField::Subject => "題名",
        DraftField::Description => "説明",
        DraftField::Tracker => "トラッカー",
        DraftField::Status => "ステータス",
        DraftField::Priority => "優先度",
        DraftField::AssignedTo => "担当者",
        DraftField::TargetVersion => "対象バージョン",
        DraftField::StartDate => "開始日",
        DraftField::DueDate => "期日",
        DraftField::DoneRatio => "進捗率",
        DraftField::EstimatedHours => "予定工数",
        DraftField::Category => "カテゴリー",
    }
}

/// 項目の表示値。値のない項目は`-`、Redmineの既定値にする項目は`(既定)`と表示する。
fn display_value(field: DraftField, draft: &NewIssueDraft, store: &Store) -> String {
    fn or_dash(value: Option<String>) -> String {
        value.unwrap_or_else(|| "-".to_string())
    }
    fn or_default(value: Option<String>) -> String {
        value.unwrap_or_else(|| "(既定)".to_string())
    }
    let date = |date: Option<chrono::DateTime<chrono::Local>>| {
        or_dash(date.map(|date| date.format("%Y/%m/%d").to_string()))
    };
    match field {
        DraftField::Subject => draft.subject.clone(),
        DraftField::Description => or_dash(draft.description.lines().next().map(|line| {
            match draft.description.lines().count() {
                1 => line.to_string(),
                count => format!("{line} (全{count}行)"),
            }
        })),
        DraftField::Tracker => or_default(
            draft
                .tracker_id
                .and_then(|id| store.get_tracker(id))
                .map(|tracker| tracker.name.clone()),
        ),
        DraftField::Status => or_default(
            draft
                .status_id
                .and_then(|id| store.get_issue_status(id))
                .map(|status| status.name.clone()),
        ),
        DraftField::Priority => or_default(
            draft
                .priority_id
                .and_then(|id| store.get_priority(id))
                .map(|priority| priority.name.clone()),
        ),
        DraftField::AssignedTo => or_dash(
            draft
                .assigned_to_id
                .and_then(|id| store.get_user(id))
                .map(|user| user.name.clone()),
        ),
        DraftField::TargetVersion => or_dash(
            draft
                .target_version_id
                .and_then(|id| store.get_target_version(id))
                .map(|version| version.name.clone()),
        ),
        DraftField::StartDate => date(draft.start_date),
        DraftField::DueDate => date(draft.due_date),
        DraftField::DoneRatio => or_default(draft.done_ratio.map(|ratio| format!("{ratio}%"))),
        DraftField::EstimatedHours => {
            or_dash(draft.estimated_hours.map(|hours| format!("{hours} h")))
        }
        DraftField::Category => or_dash(
            draft
                .category_id
                .and_then(|id| store.get_category(id))
                .map(|category| category.name.clone()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{KeyEvent, KeyModifiers};
    use crate::stores::Dispatcher;
    use crate::test_support::sample_issue_aggregate;
    use crate::test_support::{dispatch_loaded_issue, dispatch_sample_masters, render_snapshot};

    fn key(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn loaded_dispatcher() -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        dispatch_sample_masters(&mut dispatcher);
        while dispatcher.consume_actinos_len() > 0 {
            dispatcher.consume_action();
        }
        dispatch_loaded_issue(
            &mut dispatcher,
            sample_issue_aggregate(3, "親のIssue", 1.into(), None, None, None, 0),
        );
        dispatcher
    }

    #[test]
    fn focus_moves_between_fields_and_the_add_attachment_row_within_bounds() {
        let mut component = NewIssueComponent::new(ProjectId::new(1), None);

        assert_eq!(
            component.process_event(&key(KeyCode::Char('k'))),
            Some(EventProcessResult::Handled)
        );
        assert_eq!(component.focused_index, 0);
        for _ in 0..20 {
            component.process_event(&key(KeyCode::Char('j')));
        }
        assert_eq!(component.focused_index, FIELDS.len());
        assert_eq!(component.process_event(&key(KeyCode::Char('x'))), None);
    }

    #[test]
    fn enter_requests_editing_the_focused_field() {
        let mut component = NewIssueComponent::new(ProjectId::new(1), None);
        component.process_event(&key(KeyCode::Char('j')));

        assert_eq!(
            component.process_event(&key(KeyCode::Enter)),
            Some(EventProcessResult::EditFieldRequested {
                field: DraftField::Description
            })
        );
    }

    #[test]
    fn select_box_observers_write_the_selection_back_to_the_draft() {
        let dispatcher = loaded_dispatcher();
        let component = NewIssueComponent::new(ProjectId::new(1), None);

        for field in [
            DraftField::Tracker,
            DraftField::Status,
            DraftField::Priority,
            DraftField::AssignedTo,
            DraftField::TargetVersion,
            DraftField::DoneRatio,
            DraftField::Category,
        ] {
            let mut select_box = component
                .select_box(dispatcher.store(), field)
                .unwrap_or_else(|| panic!("{field:?} should be selected from a select box"));
            (select_box.observer)(Some(1));
        }

        let draft = component.draft();
        let draft = draft.borrow();
        assert_eq!(draft.tracker_id, Some(TrackerId::new(1)));
        assert_eq!(draft.status_id, Some(IssueStatusId::new(1)));
        assert_eq!(draft.priority_id, Some(PriorityId::new(1)));
        assert_eq!(draft.assigned_to_id, Some(UserId::new(1)));
        assert_eq!(draft.target_version_id, Some(TargetVersionId::new(1)));
        assert_eq!(draft.done_ratio, Some(1));
        assert_eq!(draft.category_id, Some(CategoryId::new(1)));
    }

    #[test]
    fn select_box_none_returns_the_field_to_the_server_default() {
        let dispatcher = loaded_dispatcher();
        let component = NewIssueComponent::new(ProjectId::new(1), None);
        component.draft().borrow_mut().tracker_id = Some(TrackerId::new(2));

        let mut select_box = component
            .select_box(dispatcher.store(), DraftField::Tracker)
            .unwrap();
        assert_eq!(select_box.focused_index, 1);
        (select_box.observer)(None);

        assert_eq!(component.draft().borrow().tracker_id, None);
    }

    #[test]
    fn text_and_input_fields_have_no_select_box() {
        let dispatcher = loaded_dispatcher();
        let component = NewIssueComponent::new(ProjectId::new(1), None);

        for field in [
            DraftField::Subject,
            DraftField::Description,
            DraftField::StartDate,
            DraftField::DueDate,
            DraftField::EstimatedHours,
        ] {
            assert!(component.select_box(dispatcher.store(), field).is_none());
        }
    }

    #[test]
    fn y_requests_the_issue_select_popup() {
        let mut component = NewIssueComponent::new(ProjectId::new(1), None);

        assert_eq!(
            component.process_event(&key(KeyCode::Char('y'))),
            Some(EventProcessResult::OpenIssueSelectPopup)
        );
    }

    #[test]
    fn each_component_has_its_own_creation_id() {
        let first = NewIssueComponent::new(ProjectId::new(1), None);
        let second = NewIssueComponent::new(ProjectId::new(1), None);

        assert_ne!(first.creation_id(), second.creation_id());
    }

    #[test]
    fn snapshot_draft_resolves_names_from_the_store() {
        let dispatcher = loaded_dispatcher();
        let component = NewIssueComponent::new(ProjectId::new(1), Some(IssueId::new(3)));
        {
            let draft = component.draft();
            let mut draft = draft.borrow_mut();
            draft.subject = "ログイン画面の不具合".to_string();
            draft.description = "再現手順\n1. ログインする".to_string();
            draft.tracker_id = Some(TrackerId::new(1));
            draft.priority_id = Some(PriorityId::new(3));
            draft.assigned_to_id = Some(UserId::new(1001));
            draft.estimated_hours = Some(1.5);
        }

        render_snapshot(
            "new_issue_component_resolves_names",
            60,
            22,
            component.create_widget(dispatcher.store()),
        );
    }
}
