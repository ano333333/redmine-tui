use crossterm::event::Event;
use ratatui::layout::{Position, Rect, Size};

use crate::entities::IssueAggregate;
use crate::vos::{EntityIdValue, IssuePropertyDiff};
use crate::widgets::{VerticalScrollWidget, VerticalScrollWidgetState};

use super::focus_state::{EventProcessResult as RawEventProcessResult, FocusState, FocusTarget};
use super::widget::{
    IssuePropertyConflictFocus, IssuePropertyConflictRow, IssuePropertyConflictWidget,
};

pub enum EventProcessResult {
    Canceled,
    Continued { diffs: Vec<IssuePropertyDiff> },
}

pub struct IssuePropertyConflictComponent {
    server_issue: IssueAggregate,
    diffs: Vec<IssuePropertyDiff>,
    selected_choices: Vec<IssuePropertyConflictFocus>,
    focus_state: FocusState,
    vertical_scroll_state: VerticalScrollWidgetState,
}

impl IssuePropertyConflictComponent {
    /// サーバーの現在値と競合解決対象の差分からComponentを作成する。
    ///
    /// 渡された差分は、サーバー現在値からローカル編集後値へ変更する候補として扱う。
    pub fn new(server_issue: IssueAggregate, diffs: Vec<IssuePropertyDiff>) -> Self {
        let selected_choices = vec![IssuePropertyConflictFocus::After; diffs.len()];
        Self {
            server_issue,
            focus_state: FocusState::new(diffs.len()),
            vertical_scroll_state: VerticalScrollWidgetState::new(),
            diffs,
            selected_choices,
        }
    }

    /// 描画領域に合わせてスクロール状態を更新する。
    pub fn update(&mut self, area: Rect) {
        let cursor = self.cursor_global_position(area.width);
        self.vertical_scroll_state.update(cursor, area.height);
    }

    /// キーイベントを処理し、popupの終了や続行が必要な場合は結果を返す。
    pub fn process_event(&mut self, event: Event) -> Option<EventProcessResult> {
        self.focus_state
            .process_event(event)
            .and_then(|result| match result {
                RawEventProcessResult::Selected { row_index, choice } => {
                    if let Some(selected_choice) = self.selected_choices.get_mut(row_index) {
                        *selected_choice = choice;
                    }
                    None
                }
                RawEventProcessResult::Canceled => Some(EventProcessResult::Canceled),
                RawEventProcessResult::Continued => Some(EventProcessResult::Continued {
                    diffs: self.resolved_diffs(),
                }),
            })
    }

    /// 現在の状態を反映したWidgetを作成する。
    pub fn create_widget(&self, area: Rect) -> VerticalScrollWidget<'_> {
        let rows = self.widget_rows();
        let widget = IssuePropertyConflictWidget::new(&rows)
            .with_focused_button(self.focus_state.focused_button());
        let line_count = widget.line_count(area.width) as u16;
        let mut scroll_widget = VerticalScrollWidget::new(
            &self.vertical_scroll_state,
            Size::new(area.width, area.height),
        );
        scroll_widget.render_widget(widget, line_count);
        scroll_widget
    }

    /// スクロール適用後の画面上カーソル位置を返す。
    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        let cursor = self.cursor_global_position(area.width);
        let position = self
            .vertical_scroll_state
            .calc_cursor_area_position(cursor, area);
        if position.y >= area.y && position.y < area.y.saturating_add(area.height) {
            Some(position)
        } else {
            None
        }
    }

    fn widget_rows(&self) -> Vec<IssuePropertyConflictRow> {
        self.diffs
            .iter()
            .zip(self.selected_choices.iter().copied())
            .map(|(diff, choice)| row_from_diff(&self.server_issue, diff, choice))
            .collect()
    }

    fn resolved_diffs(&self) -> Vec<IssuePropertyDiff> {
        self.diffs
            .iter()
            .cloned()
            .zip(self.selected_choices.iter().copied())
            .filter_map(|(diff, choice)| match choice {
                IssuePropertyConflictFocus::After => Some(diff),
                IssuePropertyConflictFocus::Server => None,
            })
            .collect()
    }

    fn cursor_global_position(&self, width: u16) -> Position {
        let rows = self.widget_rows();
        let widget = IssuePropertyConflictWidget::new(&rows)
            .with_focused_button(self.focus_state.focused_button());
        match self.focus_state.target() {
            FocusTarget::Cell { row_index, column } => {
                widget.cursor_position_for_cell(row_index, column, width)
            }
            FocusTarget::Button(button) => widget.cursor_position_for_button(button, width),
        }
    }
}

fn row_from_diff(
    server_issue: &IssueAggregate,
    diff: &IssuePropertyDiff,
    focused_choice: IssuePropertyConflictFocus,
) -> IssuePropertyConflictRow {
    let value = diff_value_text(server_issue, diff);
    if matches!(diff, IssuePropertyDiff::Description(_)) {
        IssuePropertyConflictRow::new_markdown(
            property_name(diff),
            value.before,
            value.after,
            value.server,
            focused_choice,
        )
    } else {
        IssuePropertyConflictRow::new(
            property_name(diff),
            value.before,
            value.after,
            value.server,
            focused_choice,
        )
    }
}

struct DiffValueText {
    before: String,
    after: String,
    server: String,
}

fn diff_value_text(server_issue: &IssueAggregate, diff: &IssuePropertyDiff) -> DiffValueText {
    match diff {
        IssuePropertyDiff::Subject(diff) => {
            diff_text(&diff.before, &diff.after, &server_issue.issue.subject)
        }
        IssuePropertyDiff::AuthorId(diff) => {
            id_diff_text(diff.before, diff.after, server_issue.author_id)
        }
        IssuePropertyDiff::CreatedOn(diff) => {
            diff_text(&diff.before, &diff.after, &server_issue.created_on)
        }
        IssuePropertyDiff::UpdatedOn(diff) => {
            diff_text(&diff.before, &diff.after, &server_issue.updated_on)
        }
        IssuePropertyDiff::ProjectId(diff) => {
            id_diff_text(diff.before, diff.after, server_issue.issue.project_id)
        }
        IssuePropertyDiff::TrackerId(diff) => {
            id_diff_text(diff.before, diff.after, server_issue.tracker_id)
        }
        IssuePropertyDiff::StatusId(diff) => {
            id_diff_text(diff.before, diff.after, server_issue.issue.status_id)
        }
        IssuePropertyDiff::PriorityId(diff) => {
            id_diff_text(diff.before, diff.after, server_issue.priority_id)
        }
        IssuePropertyDiff::AssignedToId(diff) => {
            option_id_diff_text(diff.before, diff.after, server_issue.assigned_to_id)
        }
        IssuePropertyDiff::TargetVersionId(diff) => {
            option_id_diff_text(diff.before, diff.after, server_issue.target_version_id)
        }
        IssuePropertyDiff::FixedVersion(_) => {
            panic!("サーバーIssueにfixed_version propertyがないため表示できません")
        }
        IssuePropertyDiff::StartDate(diff) => {
            option_diff_text(&diff.before, &diff.after, &server_issue.start_date)
        }
        IssuePropertyDiff::DueDate(diff) => {
            option_diff_text(&diff.before, &diff.after, &server_issue.due_date)
        }
        IssuePropertyDiff::DoneRatio(diff) => {
            diff_text(&diff.before, &diff.after, &server_issue.done_ratio)
        }
        IssuePropertyDiff::EstimatedHours(diff) => {
            option_diff_text(&diff.before, &diff.after, &server_issue.estimated_hours)
        }
        IssuePropertyDiff::TotalSpentHours(diff) => {
            option_diff_text(&diff.before, &diff.after, &server_issue.total_spent_hours)
        }
        IssuePropertyDiff::ResolveWay(_) => {
            panic!("サーバーIssueにresolve_way propertyがないため表示できません")
        }
        IssuePropertyDiff::CategoryId(diff) => {
            option_id_diff_text(diff.before, diff.after, server_issue.category_id)
        }
        IssuePropertyDiff::Description(diff) => {
            diff_text(&diff.before, &diff.after, &server_issue.issue.description)
        }
        IssuePropertyDiff::ChildIds(diff) => diff_text(
            &diff
                .before
                .iter()
                .map(|id| id.get().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            &diff
                .after
                .iter()
                .map(|id| id.get().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            &server_issue
                .child_ids
                .iter()
                .map(|id| id.get().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
    }
}

fn diff_text(
    before: &impl ToString,
    after: &impl ToString,
    server: &impl ToString,
) -> DiffValueText {
    let before = before.to_string();
    let after = after.to_string();
    DiffValueText {
        server: server.to_string(),
        before,
        after,
    }
}

fn id_diff_text(
    before: impl EntityIdValue,
    after: impl EntityIdValue,
    server: impl EntityIdValue,
) -> DiffValueText {
    diff_text(&before.get(), &after.get(), &server.get())
}

fn option_id_diff_text(
    before: Option<impl EntityIdValue>,
    after: Option<impl EntityIdValue>,
    server: Option<impl EntityIdValue>,
) -> DiffValueText {
    diff_text(
        &option_id_text(before),
        &option_id_text(after),
        &option_id_text(server),
    )
}

fn option_id_text(id: Option<impl EntityIdValue>) -> String {
    id.map(|id| id.get().to_string())
        .unwrap_or_else(|| "(なし)".to_string())
}

fn option_diff_text<T: ToString>(
    before: &Option<T>,
    after: &Option<T>,
    server: &Option<T>,
) -> DiffValueText {
    diff_text(
        &option_text(before),
        &option_text(after),
        &option_text(server),
    )
}

fn option_text<T: ToString>(value: &Option<T>) -> String {
    value
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "(なし)".to_string())
}

fn property_name(diff: &IssuePropertyDiff) -> &'static str {
    match diff {
        IssuePropertyDiff::Subject(_) => "題名",
        IssuePropertyDiff::AuthorId(_) => "作成者",
        IssuePropertyDiff::CreatedOn(_) => "作成日時",
        IssuePropertyDiff::UpdatedOn(_) => "更新日時",
        IssuePropertyDiff::ProjectId(_) => "プロジェクト",
        IssuePropertyDiff::TrackerId(_) => "トラッカー",
        IssuePropertyDiff::StatusId(_) => "ステータス",
        IssuePropertyDiff::PriorityId(_) => "優先度",
        IssuePropertyDiff::AssignedToId(_) => "担当者",
        IssuePropertyDiff::TargetVersionId(_) => "対象バージョン",
        IssuePropertyDiff::FixedVersion(_) => "修正バージョン",
        IssuePropertyDiff::StartDate(_) => "開始日",
        IssuePropertyDiff::DueDate(_) => "期日",
        IssuePropertyDiff::DoneRatio(_) => "進捗率",
        IssuePropertyDiff::EstimatedHours(_) => "予定工数",
        IssuePropertyDiff::TotalSpentHours(_) => "作業時間",
        IssuePropertyDiff::ResolveWay(_) => "解決方法",
        IssuePropertyDiff::CategoryId(_) => "カテゴリ",
        IssuePropertyDiff::Description(_) => "説明",
        IssuePropertyDiff::ChildIds(_) => "子チケット",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::{Position, Rect};

    use crate::test_support::render_snapshot;
    use crate::test_support::sample_issue_aggregate;

    use crate::vos::IssuePropertyDiff;
    use crate::vos::issue_property_diff::{IssueDescriptionDiff, IssueStatusIdDiff};

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn diffs() -> Vec<IssuePropertyDiff> {
        vec![
            IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: 1.into(),
                after: 2.into(),
            }),
            IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "server body".to_string(),
                after: "# local body".to_string(),
            }),
        ]
    }

    fn component(diffs: Vec<IssuePropertyDiff>) -> IssuePropertyConflictComponent {
        let mut server_issue =
            sample_issue_aggregate(1, "server subject", 1.into(), None, None, None, 0);
        server_issue.issue.description = "server body".to_string();
        IssuePropertyConflictComponent::new(server_issue, diffs)
    }

    #[test]
    fn diff_value_text_uses_the_server_issue_value() {
        let mut server_issue =
            sample_issue_aggregate(1, "server subject", 9.into(), None, None, None, 0);
        server_issue.issue.description = "# server body".to_string();

        let status = diff_value_text(
            &server_issue,
            &IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                before: 1.into(),
                after: 2.into(),
            }),
        );
        let description = diff_value_text(
            &server_issue,
            &IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "original body".to_string(),
                after: "# local body".to_string(),
            }),
        );

        assert_eq!(status.server, "9");
        assert_eq!(description.server, "# server body");
    }

    #[test]
    fn new_focuses_first_after_cell() {
        let mut component = component(diffs());

        component.update(AREA);

        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 39, y: 3 })
        );
    }

    #[test]
    fn j_and_k_move_cursor_between_rows() {
        let mut component = component(diffs());
        component.update(AREA);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('j')))
                .is_none()
        );
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 39, y: 4 })
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('k')))
                .is_none()
        );
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 39, y: 3 })
        );
    }

    #[test]
    fn l_and_h_move_cursor_between_after_and_server_columns() {
        let mut component = component(diffs());
        component.update(AREA);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('l')))
                .is_none()
        );
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 60, y: 3 })
        );

        assert!(
            component
                .process_event(key_event(KeyCode::Char('h')))
                .is_none()
        );
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 39, y: 3 })
        );
    }

    #[test]
    fn enter_on_cell_changes_resolved_diffs_returned_by_continue() {
        let mut component = component(diffs());

        component.process_event(key_event(KeyCode::Char('l')));
        let result = component.process_event(key_event(KeyCode::Enter));

        assert!(result.is_none());
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));

        let Some(EventProcessResult::Continued { diffs }) =
            component.process_event(key_event(KeyCode::Enter))
        else {
            panic!("continue should return selected diffs");
        };
        assert_eq!(
            diffs,
            vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "server body".to_string(),
                after: "# local body".to_string(),
            })]
        );
    }

    #[test]
    fn j_from_last_row_focuses_continue_button_and_h_moves_to_cancel() {
        let mut component = component(diffs());
        component.update(AREA);

        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 71, y: 7 })
        );

        component.process_event(key_event(KeyCode::Char('h')));
        component.update(AREA);
        assert_eq!(
            component.cursor_position(AREA),
            Some(Position { x: 56, y: 7 })
        );
    }

    #[test]
    fn q_and_enter_on_cancel_request_popup_close() {
        let mut component = component(diffs());

        assert!(matches!(
            component.process_event(key_event(KeyCode::Char('q'))),
            Some(EventProcessResult::Canceled)
        ));

        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('h')));
        assert!(matches!(
            component.process_event(key_event(KeyCode::Enter)),
            Some(EventProcessResult::Canceled)
        ));
    }

    #[test]
    fn enter_on_continue_returns_selected_after_diffs_and_omits_server_choices() {
        let mut component = component(diffs());

        component.process_event(key_event(KeyCode::Char('l')));
        component.process_event(key_event(KeyCode::Enter));
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));
        let result = component.process_event(key_event(KeyCode::Enter));

        let Some(EventProcessResult::Continued { diffs }) = result else {
            panic!("continue should return selected diffs");
        };
        assert_eq!(
            diffs,
            vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "server body".to_string(),
                after: "# local body".to_string(),
            })]
        );
    }

    #[test]
    fn update_keeps_cursor_visible_when_cursor_moves_outside_visible_area() {
        let many_rows = (0..10)
            .map(|index| {
                IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                    before: (index + 1).into(),
                    after: (index + 2).into(),
                })
            })
            .collect();
        let mut component = component(many_rows);

        component.update(AREA);
        for _ in 0..8 {
            component.process_event(key_event(KeyCode::Char('j')));
            component.update(AREA);
        }

        let cursor = component
            .cursor_position(AREA)
            .expect("cursor should be visible after scroll update");
        assert!(cursor.y >= AREA.y);
        assert!(cursor.y < AREA.y + AREA.height);
    }

    #[test]
    fn snapshot_create_widget_renders_vertical_scroll_widget_with_button_focus() {
        let mut component = component(diffs());
        component.process_event(key_event(KeyCode::Char('j')));
        component.process_event(key_event(KeyCode::Char('j')));
        component.update(AREA);

        render_snapshot(
            "issue_property_conflict_component_vertical_scroll_widget_continue_focus",
            AREA.width,
            AREA.height,
            component.create_widget(AREA),
        );
    }
}
