use ratatui::buffer::Buffer;
use std::cmp::min;

use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::journals_list_item::JournalItemWidget;

pub struct JournalsListWidget<'a> {
    widgets: Vec<JournalItemWidget<'a>>,
}

impl<'a> JournalsListWidget<'a> {
    pub fn new(widgets: Vec<JournalItemWidget<'a>>) -> Self {
        Self { widgets }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.widgets
            .iter()
            .map(|widget| widget.line_count(width))
            .sum()
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
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Local};

    use super::*;
    use crate::components::issue::detail::journals_list::journals_list_item::JournalItemWidgetState;
    use crate::components::issue::detail::journals_list::journals_list_item::widget::ResolvedJournalDetail;
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

    fn assigned_to_detail() -> ResolvedJournalDetail {
        ResolvedJournalDetail {
            field_label: "担当者",
            old_display: "(なし)".to_string(),
            new_display: "bob".to_string(),
        }
    }

    fn status_detail() -> ResolvedJournalDetail {
        ResolvedJournalDetail {
            field_label: "ステータス",
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
        state.update(24, &user, &updated_on, &notes);
        let journal = create_journal(user, updated_on, &notes);
        let journals = vec![JournalItemWidget::new(
            &journal,
            vec![assigned_to_detail()],
            &state,
            true,
        )];
        let line_count = JournalsListWidget::new(journals).line_count(24);
        let journals = vec![JournalItemWidget::new(
            &journal,
            vec![assigned_to_detail()],
            &state,
            true,
        )];

        render_snapshot(
            "journals_list_mixed_entries",
            24,
            line_count,
            JournalsListWidget::new(journals),
        );
    }

    #[test]
    fn snapshot_journals_list_clips_without_relayout_when_height_is_short() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &user, &updated_on, &notes);
        let journal = create_journal(user, updated_on, &notes);
        let journals = vec![JournalItemWidget::new(
            &journal,
            vec![assigned_to_detail()],
            &state,
            true,
        )];

        render_snapshot(
            "journals_list_clipped_height",
            24,
            5,
            JournalsListWidget::new(journals),
        );
    }

    #[test]
    fn line_count_journals_list_current_values() {
        let user = "alice".to_string();
        let updated_on = local_datetime("2026-01-15T00:00:00+09:00");
        let notes = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &user, &updated_on, &notes);
        let journal = create_journal(user, updated_on, &notes);
        let journals = vec![JournalItemWidget::new(
            &journal,
            vec![assigned_to_detail()],
            &state,
            false,
        )];
        let widget = JournalsListWidget::new(journals);
        assert_eq!(widget.line_count(24), 9);
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
            state.update(32, &user, &updated_on, &short_notes);
            let journal = create_journal(user.clone(), updated_on, &short_notes);
            let journals = vec![JournalItemWidget::new(
                &journal,
                vec![assigned_to_detail()],
                &state,
                false,
            )];
            JournalsListWidget::new(journals).line_count(32)
        };

        let narrow_short = {
            let mut state = JournalItemWidgetState::new();
            state.update(18, &user, &updated_on, &short_notes);
            let journal = create_journal(user.clone(), updated_on, &short_notes);
            let journals = vec![JournalItemWidget::new(
                &journal,
                vec![assigned_to_detail()],
                &state,
                false,
            )];
            JournalsListWidget::new(journals).line_count(18)
        };

        let narrow_long = {
            let mut state = JournalItemWidgetState::new();
            state.update(18, &user, &updated_on, &long_notes);
            let journal = create_journal(user.clone(), updated_on, &long_notes);
            let journals = vec![JournalItemWidget::new(
                &journal,
                vec![assigned_to_detail()],
                &state,
                false,
            )];
            JournalsListWidget::new(journals).line_count(18)
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
        let widget = JournalItemWidget::new(&journal, details, &state, false);
        assert_eq!(widget.line_count(20), 6);
    }
}
