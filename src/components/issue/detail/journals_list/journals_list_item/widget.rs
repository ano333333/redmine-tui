use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span, Stylize};
use ratatui::style::Color;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::stores::RemoteJournalState;

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const EMPTY_NOTES_PLACEHOLDER: &str = "(none)";

/// JournalDetailAttrをStoreで解決した、表示用の値。WidgetはStoreを知らない。
pub struct ResolvedJournalDetail {
    pub field_label: &'static str,
    pub old_display: String,
    pub new_display: String,
}

/// Remote Journalの永続entityとローカルの編集状態を描画用に統合した参照。
pub struct RemoteJournalItemView<'a> {
    pub user: &'a str,
    pub updated_on: &'a chrono::DateTime<chrono::Local>,
    /// 編集開始時にも使う表示中のnotes。本文の描画自体は事前計算済みのbufferが担う。
    pub notes: &'a str,
    pub state_marker: &'static str,
}

/// 同期済みの場合は空文字列、未保存の状態ではヘッダーへ付加するラベルを返す。
pub fn state_marker(state: &RemoteJournalState) -> &'static str {
    match state {
        RemoteJournalState::Synced => "",
        RemoteJournalState::Edited { .. } => "(edited)",
        RemoteJournalState::Uploading { .. } => "(uploading)",
    }
}

/// 同期済みなら取得時のnotes、編集済みまたはupload中なら未保存の編集結果を返す。
pub fn display_notes<'a>(journal_notes: &'a str, state: &'a RemoteJournalState) -> &'a str {
    match state {
        RemoteJournalState::Synced => journal_notes,
        RemoteJournalState::Edited { diff, .. } => &diff.after,
        RemoteJournalState::Uploading { diff, .. } => &diff.after,
    }
}

pub struct JournalItemWidgetState {
    comment_buffer: Buffer,
    hash: u64,
}

impl JournalItemWidgetState {
    pub fn new() -> Self {
        Self {
            comment_buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
            hash: 0,
        }
    }

    pub fn update(&mut self, width: u16, user: &str, updated_on: &DateTime<Local>, notes: &str) {
        let mut hasher = DefaultHasher::new();
        user.hash(&mut hasher);
        updated_on.hash(&mut hasher);
        notes.hash(&mut hasher);
        let hash = hasher.finish();

        if self.comment_buffer.area.width != width || self.hash != hash {
            self.comment_buffer = render_comment_in_buffer(width, user, updated_on, notes);
            self.hash = hash;
        }
    }

    pub fn comment_line_count(&self) -> u16 {
        self.comment_buffer.area.height
    }
}

pub struct JournalItemWidget<'a> {
    view: RemoteJournalItemView<'a>,
    details: Vec<ResolvedJournalDetail>,
    comment_state: &'a JournalItemWidgetState,
    focused: bool,
}

impl<'a> Widget for JournalItemWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let property_height = self.details.len() as u16 + 3;
        let property = create_property_paragraph(
            self.view.user,
            &self.details,
            self.view.updated_on,
            self.view.state_marker,
        );

        let property_area = Rect::new(
            area.x,
            area.y,
            area.width,
            min(area.height, property_height),
        );
        if property_area.height > 0 {
            property.render(property_area, buf);
        }

        let buf_src = &self.comment_state.comment_buffer;
        let comment_y = area.y.saturating_add(property_height);
        let comment_height = area
            .height
            .saturating_sub(property_height)
            .min(buf_src.area.height);
        let width = min(buf_src.area.width, area.width);
        let height = comment_height;
        for y in 0..height {
            for x in 0..width {
                let Some(src_cell) = buf_src.cell((x, y)).cloned() else {
                    continue;
                };
                let dst_x = area.x + x;
                let dst_y = comment_y + y;
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    *dst_cell = src_cell;
                }
            }
        }

        if self.focused {
            for y in 0..area.height {
                for x in 0..area.width {
                    if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                        cell.set_bg(FOCUS_BG);
                    }
                }
            }
        }
    }
}

impl<'a> JournalItemWidget<'a> {
    pub fn new(
        view: RemoteJournalItemView<'a>,
        details: Vec<ResolvedJournalDetail>,
        comment_state: &'a JournalItemWidgetState,
        focused: bool,
    ) -> Self {
        Self {
            view,
            details,
            comment_state,
            focused,
        }
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.details.len() as u16 + 1 + self.comment_state.comment_line_count() + 1
    }
}

fn create_property_paragraph(
    user: &str,
    details: &[ResolvedJournalDetail],
    updated_on: &DateTime<Local>,
    state_marker: &str,
) -> Paragraph<'static> {
    let title = create_header(user, updated_on, state_marker);
    let mut lines = vec![title, Line::from("")];
    for detail in details {
        let line = Line::from(vec![
            Span::from("  ・ "),
            Span::from(detail.field_label).bold(),
            Span::from(" を "),
            Span::from(detail.old_display.clone()).italic(),
            Span::from(" から "),
            Span::from(detail.new_display.clone()).italic(),
            Span::from(" に変更"),
        ])
        .gray();
        lines.push(line);
    }
    lines.push(Line::from(""));
    Paragraph::new(Text::from(lines))
}

fn create_header(creator: &str, updated_at: &DateTime<Local>, state_marker: &str) -> Line<'static> {
    let mut spans = vec![
        Span::from(creator.to_owned()).blue(),
        Span::from("が"),
        Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
        Span::from("に更新"),
    ];
    if !state_marker.is_empty() {
        // 取得済みのメタデータと区別できるよう、ローカルで遷移する未保存状態を警告色にする。
        spans.push(Span::from(format!(" {}", state_marker)).yellow());
    }
    Line::from(spans)
}

fn render_comment_in_buffer(width: u16, _: &str, _: &DateTime<Local>, body: &str) -> Buffer {
    let body = if body.is_empty() {
        EMPTY_NOTES_PLACEHOLDER
    } else {
        body
    };
    let body = Paragraph::new(tui_markdown::from_str(body)).wrap(Wrap { trim: true });
    let body_line_count = body.line_count(width) as u16;
    let area = Rect::new(0, 0, width, body_line_count);
    let mut buffer = Buffer::empty(area);
    body.render(area, &mut buffer);
    buffer
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Local};

    use super::*;
    use crate::{
        entities::Journal,
        test_support::{local_datetime, render_snapshot},
        vos::{IssueId, JournalId},
    };

    fn create_journal(user: String, updated_on: DateTime<Local>, notes: &str) -> Journal {
        Journal {
            id: JournalId::new(1),
            issue_id: IssueId::new(1),
            user,
            updated_on,
            details: vec![],
            notes: notes.to_owned(),
        }
    }

    fn view_of<'v>(
        journal: &'v Journal,
        notes: &'v str,
        state_marker: &'static str,
    ) -> RemoteJournalItemView<'v> {
        RemoteJournalItemView {
            user: &journal.user,
            updated_on: &journal.updated_on,
            notes,
            state_marker,
        }
    }

    fn assigned_to_detail(old: Option<&str>, new: Option<&str>) -> ResolvedJournalDetail {
        ResolvedJournalDetail {
            field_label: "担当者",
            old_display: old.map(str::to_string).unwrap_or("(なし)".to_string()),
            new_display: new.map(str::to_string).unwrap_or("(なし)".to_string()),
        }
    }

    fn status_detail(old: &str, new: &str) -> ResolvedJournalDetail {
        ResolvedJournalDetail {
            field_label: "ステータス",
            old_display: old.to_string(),
            new_display: new.to_string(),
        }
    }

    #[test]
    fn line_count_includes_expected_blank_lines_and_wrapped_comment() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![
            status_detail("新規", "進行中"),
            assigned_to_detail(None, Some("bob")),
        ];
        let notes = "short line\n\nwrapping words for the comment area".to_string();
        let width = 20;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &creator, &updated_at, &notes);

        let journal = create_journal(creator, updated_at, &notes);
        let view = view_of(&journal, &notes, "");
        let widget = JournalItemWidget::new(view, properties, &state, false);

        assert_eq!(widget.line_count(width), 10);
    }

    #[test]
    fn line_count_for_empty_notes_includes_placeholder_line() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "".to_string();
        let width = 20;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &creator, &updated_at, &notes);

        let journal = create_journal(creator, updated_at, &notes);
        let view = view_of(&journal, &notes, "");
        let widget = JournalItemWidget::new(view, properties, &state, false);

        assert_eq!(state.comment_line_count(), 1);
        assert_eq!(widget.line_count(width), 6);
    }

    #[test]
    fn snapshot_journal_item_matches_expected_section_order() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &user, &updated_on, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "");
        let widget = JournalItemWidget::new(view, details, &state, true);
        let line_count = widget.line_count(width);

        render_snapshot("journal_item_expected_layout", width, line_count, widget);
    }

    #[test]
    fn snapshot_journal_item_clips_without_relayout_when_height_is_short() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &user, &updated_on, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "");
        let widget = JournalItemWidget::new(view, details, &state, true);

        render_snapshot("journal_item_clipped_height", width, 5, widget);
    }

    #[test]
    fn snapshot_state_marker_uploading() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "uploading notes for the state marker snapshot".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &user, &updated_on, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "(uploading)");
        let widget = JournalItemWidget::new(view, details, &state, true);
        let line_count = widget.line_count(width);

        render_snapshot(
            "journal_item_state_marker_uploading",
            width,
            line_count,
            widget,
        );
    }

    #[test]
    fn snapshot_journal_item_empty_notes_renders_placeholder() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &user, &updated_on, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "");
        let widget = JournalItemWidget::new(view, details, &state, true);
        let line_count = widget.line_count(width);

        render_snapshot(
            "journal_item_empty_notes_placeholder",
            width,
            line_count,
            widget,
        );
    }
}
