use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span, Stylize};
use ratatui::style::Color;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::entities::{Journal, JournalDetail, JournalDetailAttr};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

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

    pub fn update(
        &mut self,
        width: u16,
        user: &String,
        updated_on: &DateTime<Local>,
        notes: &String,
    ) {
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
    journal: &'a Journal,
    comment_state: &'a JournalItemWidgetState,
    focused: bool,
}

impl<'a> Widget for JournalItemWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let property_height = self.journal.details.len() as u16 + 3;
        let property = create_property_paragraph(
            &self.journal.user,
            &self.journal.details,
            &self.journal.updated_on,
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
        journal: &'a Journal,
        comment_state: &'a JournalItemWidgetState,
        focused: bool,
    ) -> Self {
        Self {
            journal,
            comment_state,
            focused,
        }
    }

    pub fn line_count(&self, _: u16) -> u16 {
        1 + 1 + self.journal.details.len() as u16 + 1 + self.comment_state.comment_line_count() + 1
    }
}

fn create_property_paragraph(
    user: &String,
    details: &[JournalDetail],
    updated_on: &DateTime<Local>,
) -> Paragraph<'static> {
    let title = create_header(user, updated_on);
    let mut lines = vec![title, Line::from("")];
    for detail in details {
        match detail {
            JournalDetail::Attr(attr) => match attr {
                JournalDetailAttr::StatusId { old, new } => {
                    let line = Line::from(vec![
                        Span::from("  ・ "),
                        Span::from("ステータス").bold(),
                        Span::from(" を "),
                        Span::from(old.clone()).italic(),
                        Span::from(" から "),
                        Span::from(new.clone()).italic(),
                        Span::from(" に変更"),
                    ])
                    .gray();
                    lines.push(line);
                }
                JournalDetailAttr::DueDate { old, new } => {
                    let line = Line::from(vec![
                        Span::from("  ・ "),
                        Span::from("期日").bold(),
                        Span::from(" を "),
                        Span::from(old.format("%Y/%m/%d").to_string()).italic(),
                        Span::from(" から "),
                        Span::from(new.format("%Y/%m/%d").to_string()).italic(),
                        Span::from(" に変更"),
                    ])
                    .gray();
                    lines.push(line);
                }
                JournalDetailAttr::AssignedTo { old, new } => {
                    let line = Line::from(vec![
                        Span::from("  ・ "),
                        Span::from("担当者").bold(),
                        Span::from(" を "),
                        Span::from(old.clone().unwrap_or("(なし)".to_string())).italic(),
                        Span::from(" から "),
                        Span::from(new.clone().unwrap_or("(なし)".to_string())).italic(),
                        Span::from(" に変更"),
                    ])
                    .gray();
                    lines.push(line);
                }
            },
        }
    }
    lines.push(Line::from(""));
    Paragraph::new(Text::from(lines))
}

fn create_header(creator: &String, updated_at: &DateTime<Local>) -> Line<'static> {
    Line::from(vec![
        Span::from(creator.clone()).blue(),
        Span::from("が"),
        Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
        Span::from("に更新"),
    ])
}

fn render_comment_in_buffer(width: u16, _: &String, _: &DateTime<Local>, body: &String) -> Buffer {
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
        entities::{Journal, JournalDetail, JournalDetailAttr},
        test_support::{local_datetime, render_snapshot},
    };

    fn create_journal(
        user: String,
        updated_on: DateTime<Local>,
        details: Vec<JournalDetail>,
        notes: &String,
    ) -> Journal {
        Journal {
            id: 1,
            user,
            updated_on,
            details,
            notes: notes.clone(),
        }
    }

    #[test]
    fn line_count_includes_expected_blank_lines_and_wrapped_comment() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![
            JournalDetail::Attr(JournalDetailAttr::StatusId {
                old: "新規".to_string(),
                new: "進行中".to_string(),
            }),
            JournalDetail::Attr(JournalDetailAttr::AssignedTo {
                old: None,
                new: Some("bob".to_string()),
            }),
        ];
        let notes = "short line\n\nwrapping words for the comment area".to_string();
        let width = 20;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &creator, &updated_at, &notes);

        let journal = create_journal(creator, updated_at, properties, &notes);
        let widget = JournalItemWidget::new(&journal, &state, false);

        assert_eq!(widget.line_count(width), 10);
    }

    #[test]
    fn snapshot_journal_item_matches_expected_section_order() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![JournalDetail::Attr(JournalDetailAttr::AssignedTo {
            old: None,
            new: Some("bob".to_string()),
        })];
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &user, &updated_on, &notes);

        let journal = create_journal(user, updated_on, details, &notes);
        let widget = JournalItemWidget::new(&journal, &state, true);
        let line_count = widget.line_count(width);

        render_snapshot("journal_item_expected_layout", width, line_count, widget);
    }

    #[test]
    fn snapshot_journal_item_clips_without_relayout_when_height_is_short() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![JournalDetail::Attr(JournalDetailAttr::AssignedTo {
            old: None,
            new: Some("bob".to_string()),
        })];
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();
        let width = 24;

        let mut state = JournalItemWidgetState::new();
        state.update(width, &user, &updated_on, &notes);

        let journal = create_journal(user, updated_on, details, &notes);
        let widget = JournalItemWidget::new(&journal, &state, true);

        render_snapshot("journal_item_clipped_height", width, 5, widget);
    }
}
