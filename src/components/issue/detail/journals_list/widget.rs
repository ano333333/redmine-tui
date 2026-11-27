use ratatui::buffer::Buffer;
use std::cmp::min;

use ratatui::layout::{Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::i18n::tr;
use crate::widgets::gutter::{Gutter, indented_area};
use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED};

use super::journals_list_item::JournalItemWidget;
use crate::components::issue::detail::markdown_view::MarkdownImagePlacement;

/// Journalの後に置く追加行と、縦線を引かない末尾の余白行。
pub const TRAILING_LINES: u16 = 2;

pub struct JournalsListWidget<'a> {
    widgets: Vec<JournalItemWidget<'a>>,
    create_button_focused: bool,
    create_button_enabled: bool,
}

impl<'a> JournalsListWidget<'a> {
    pub fn new(
        widgets: Vec<JournalItemWidget<'a>>,
        create_button_focused: bool,
        create_button_enabled: bool,
    ) -> Self {
        Self {
            widgets,
            create_button_focused,
            create_button_enabled,
        }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.widgets
            .iter()
            .map(|widget| widget.line_count(width))
            .sum::<u16>()
            + TRAILING_LINES
    }

    /// Widgetの左上から見たnotesの画像の位置。画像の行は空けて描くため、呼び出し側が画面へ直接描く。
    pub fn images(&self, width: u16) -> Vec<MarkdownImagePlacement> {
        let mut y = 0;
        let mut images = Vec::new();
        for widget in &self.widgets {
            images.extend(
                widget
                    .images()
                    .into_iter()
                    .map(|placement| MarkdownImagePlacement {
                        position: Position::new(placement.position.x, placement.position.y + y),
                        image: placement.image,
                    }),
            );
            y += widget.line_count(width);
        }
        images
    }
}

impl Widget for JournalsListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut y = area.y;
        let end_y = area.y + area.height;

        for widget in self.widgets {
            if y >= end_y {
                break;
            }

            let height = min(widget.line_count(area.width), end_y - y);
            let row = Rect::new(area.x, y, area.width, height);
            widget.render(row, buf);
            y += height;
        }

        if y < end_y {
            let row_area = Rect::new(area.x, y, area.width, 1);
            Gutter::line().render(row_area, buf);
            let color = if self.create_button_enabled {
                ACCENT
            } else {
                MUTED
            };
            Paragraph::new(Line::from(tr!("journals-add")).fg(color))
                .render(indented_area(row_area), buf);
            if self.create_button_focused {
                for x in 0..row_area.width {
                    if let Some(cell) = buf.cell_mut((row_area.x + x, row_area.y)) {
                        cell.set_bg(FOCUS_BG);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Local};

    use super::*;
    use crate::components::issue::detail::journals_list::journals_list_item::JournalItemWidgetState;
    use crate::components::issue::detail::journals_list::journals_list_item::widget::{
        RemoteJournalItemView, ResolvedJournalDetail,
    };
    use crate::{
        entities::Journal,
        test_support::{local_datetime, render_snapshot},
        vos::{IssueId, JournalId},
    };

    fn view_of<'j>(
        journal: &'j Journal,
        notes: &'j str,
        state_marker: &str,
    ) -> RemoteJournalItemView<'j> {
        RemoteJournalItemView {
            user: &journal.user,
            updated_on: journal.updated_on.as_ref(),
            notes,
            state_marker: state_marker.to_string(),
        }
    }

    fn create_journal(user: String, updated_on: DateTime<Local>, notes: &str) -> Journal {
        Journal {
            id: JournalId::new(1),
            issue_id: IssueId::new(1),
            user,
            updated_on: Some(updated_on),
            details: vec![],
            notes: notes.to_owned(),
        }
    }

    fn assigned_to_detail() -> ResolvedJournalDetail {
        ResolvedJournalDetail::Attr {
            field_label: "Assignee".to_string(),
            old_display: "(none)".to_string(),
            new_display: "bob".to_string(),
        }
    }

    fn status_detail() -> ResolvedJournalDetail {
        ResolvedJournalDetail::Attr {
            field_label: "Status".to_string(),
            old_display: "新規".to_string(),
            new_display: "進行中".to_string(),
        }
    }

    #[test]
    fn snapshot_journals_list_mixed_entries() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &user, Some(&updated_on), &notes);
        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "");
        let journals = vec![JournalItemWidget::new(
            view,
            vec![assigned_to_detail()],
            &state,
            true,
        )];
        let line_count = JournalsListWidget::new(journals, false, true).line_count(24);
        let view = view_of(&journal, &notes, "");
        let journals = vec![JournalItemWidget::new(
            view,
            vec![assigned_to_detail()],
            &state,
            true,
        )];

        render_snapshot(
            "journals_list_mixed_entries",
            24,
            line_count,
            JournalsListWidget::new(journals, false, true),
        );
    }

    #[test]
    fn snapshot_journals_list_clips_without_relayout_when_height_is_short() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &user, Some(&updated_on), &notes);
        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "");
        let journals = vec![JournalItemWidget::new(
            view,
            vec![assigned_to_detail()],
            &state,
            true,
        )];

        render_snapshot(
            "journals_list_clipped_height",
            24,
            5,
            JournalsListWidget::new(journals, false, true),
        );
    }

    #[test]
    fn line_count_journals_list_current_values() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &user, Some(&updated_on), &notes);
        let journal = create_journal(user, updated_on, &notes);
        let view = view_of(&journal, &notes, "");
        let journals = vec![JournalItemWidget::new(
            view,
            vec![assigned_to_detail()],
            &state,
            false,
        )];
        let widget = JournalsListWidget::new(journals, false, true);
        assert_eq!(widget.line_count(24), 11);
    }

    #[test]
    fn line_count_journals_list_changes_with_child_comment_height() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let short_notes = "first paragraph with enough words to wrap".to_string();
        let long_notes =
            "first paragraph\n\nsecond paragraph with wrapping words that expands the comment"
                .to_string();

        let wide_short = {
            let mut state = JournalItemWidgetState::new();
            state.update(32, &user, Some(&updated_on), &short_notes);
            let journal = create_journal(user.clone(), updated_on, &short_notes);
            let view = view_of(&journal, &short_notes, "");
            let journals = vec![JournalItemWidget::new(
                view,
                vec![assigned_to_detail()],
                &state,
                false,
            )];
            JournalsListWidget::new(journals, false, true).line_count(32)
        };

        let narrow_short = {
            let mut state = JournalItemWidgetState::new();
            state.update(18, &user, Some(&updated_on), &short_notes);
            let journal = create_journal(user.clone(), updated_on, &short_notes);
            let view = view_of(&journal, &short_notes, "");
            let journals = vec![JournalItemWidget::new(
                view,
                vec![assigned_to_detail()],
                &state,
                false,
            )];
            JournalsListWidget::new(journals, false, true).line_count(18)
        };

        let narrow_long = {
            let mut state = JournalItemWidgetState::new();
            state.update(18, &user, Some(&updated_on), &long_notes);
            let journal = create_journal(user.clone(), updated_on, &long_notes);
            let view = view_of(&journal, &long_notes, "");
            let journals = vec![JournalItemWidget::new(
                view,
                vec![assigned_to_detail()],
                &state,
                false,
            )];
            JournalsListWidget::new(journals, false, true).line_count(18)
        };

        assert!(wide_short < narrow_short);
        assert!(narrow_short < narrow_long);
    }

    #[test]
    fn line_count_property_only_increases_with_property_count() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let details = vec![status_detail(), assigned_to_detail()];

        let journal = create_journal(user, updated_on, "");
        let state = JournalItemWidgetState::new();
        let view = view_of(&journal, &journal.notes, "");
        let widget = JournalItemWidget::new(view, details, &state, false);
        assert_eq!(widget.line_count(20), 6);
    }
}
