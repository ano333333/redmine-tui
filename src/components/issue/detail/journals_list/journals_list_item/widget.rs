use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span, Stylize};
use ratatui::style::Color;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const EMPTY_NOTES_PLACEHOLDER: &str = "(none)";

/// JournalDetailAttrをStoreで解決した、表示用の値。WidgetはStoreを知らない。
pub struct ResolvedJournalDetail {
    pub field_label: &'static str,
    pub old_display: String,
    pub new_display: String,
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

    pub fn update(&mut self, width: u16, notes: &str) {
        let mut hasher = DefaultHasher::new();
        notes.hash(&mut hasher);
        let hash = hasher.finish();

        if self.comment_buffer.area.width != width || self.hash != hash {
            self.comment_buffer = render_comment_in_buffer(width, notes);
            self.hash = hash;
        }
    }

    pub fn comment_line_count(&self) -> u16 {
        self.comment_buffer.area.height
    }
}

/// Journal itemの表示model。Remote/Localの区別を型で表し、Localには
/// user・更新日時・detailsを持たせない。
pub enum JournalItemDisplay<'a> {
    Remote {
        user: &'a str,
        updated_on: &'a DateTime<Local>,
        details: Vec<ResolvedJournalDetail>,
    },
    Local,
}

pub struct JournalItemWidget<'a> {
    display: JournalItemDisplay<'a>,
    comment_state: &'a JournalItemWidgetState,
    focused: bool,
}

impl<'a> Widget for JournalItemWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let property_height = match &self.display {
            JournalItemDisplay::Remote { details, .. } => details.len() as u16 + 3,
            // 0 detailsのRemoteと同じレイアウトで、Notesは3行目から始まる
            JournalItemDisplay::Local => 3,
        };
        let property = create_property_paragraph(&self.display);

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
        display: JournalItemDisplay<'a>,
        comment_state: &'a JournalItemWidgetState,
        focused: bool,
    ) -> Self {
        Self {
            display,
            comment_state,
            focused,
        }
    }

    pub fn line_count(&self, _: u16) -> u16 {
        let detail_count = match &self.display {
            JournalItemDisplay::Remote { details, .. } => details.len() as u16,
            JournalItemDisplay::Local => 0,
        };
        1 + 1 + detail_count + 1 + self.comment_state.comment_line_count() + 1
    }
}

fn create_property_paragraph(display: &JournalItemDisplay) -> Paragraph<'static> {
    let title = match display {
        JournalItemDisplay::Remote {
            user, updated_on, ..
        } => create_header(user, updated_on),
        JournalItemDisplay::Local => Line::from("ローカルJournal"),
    };
    let mut lines = vec![title, Line::from("")];
    if let JournalItemDisplay::Remote { details, .. } = display {
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
    }
    lines.push(Line::from(""));
    Paragraph::new(Text::from(lines))
}

fn create_header(creator: &str, updated_at: &DateTime<Local>) -> Line<'static> {
    Line::from(vec![
        Span::from(creator.to_owned()).blue(),
        Span::from("が"),
        Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
        Span::from("に更新"),
    ])
}

fn render_comment_in_buffer(width: u16, body: &str) -> Buffer {
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
        vos::JournalId,
    };

    fn create_journal(user: String, updated_on: DateTime<Local>, notes: &str) -> Journal {
        Journal {
            id: JournalId::new(1),
            user,
            updated_on,
            details: vec![],
            notes: notes.to_owned(),
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

    fn remote_display(
        journal: &Journal,
        details: Vec<ResolvedJournalDetail>,
    ) -> JournalItemDisplay<'_> {
        JournalItemDisplay::Remote {
            user: &journal.user,
            updated_on: &journal.updated_on,
            details,
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
        state.update(width, &notes);

        let journal = create_journal(creator, updated_at, &notes);
        let widget = JournalItemWidget::new(remote_display(&journal, properties), &state, false);

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
        state.update(width, &notes);

        let journal = create_journal(creator, updated_at, &notes);
        let widget = JournalItemWidget::new(remote_display(&journal, properties), &state, false);

        assert_eq!(state.comment_line_count(), 1);
        assert_eq!(widget.line_count(width), 6);
    }

    #[test]
    fn line_count_for_local_journal_matches_zero_details_remote_layout() {
        let notes = "short line\n\nwrapping words for the comment area".to_string();
        let width = 20;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &notes);

        let widget = JournalItemWidget::new(JournalItemDisplay::Local, &state, false);

        assert_eq!(state.comment_line_count(), 4);
        assert_eq!(widget.line_count(width), 8);
    }

    #[test]
    fn snapshot_journal_item_matches_expected_section_order() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let widget = JournalItemWidget::new(remote_display(&journal, details), &state, true);
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
        state.update(width, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let widget = JournalItemWidget::new(remote_display(&journal, details), &state, true);

        render_snapshot("journal_item_clipped_height", width, 5, widget);
    }

    #[test]
    fn snapshot_journal_item_empty_notes_renders_placeholder() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![assigned_to_detail(None, Some("bob"))];
        let notes = "".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &notes);

        let journal = create_journal(user, updated_on, &notes);
        let widget = JournalItemWidget::new(remote_display(&journal, details), &state, true);
        let line_count = widget.line_count(width);

        render_snapshot(
            "journal_item_empty_notes_placeholder",
            width,
            line_count,
            widget,
        );
    }

    #[test]
    fn snapshot_local_journal_item_widget() {
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &notes);

        let widget = JournalItemWidget::new(JournalItemDisplay::Local, &state, true);
        let line_count = widget.line_count(width);

        render_snapshot("journal_item_local_journal", width, line_count, widget);
    }
}
