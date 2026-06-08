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
    use crate::components::issue::journals_list::journals_list_item::JournalItemWidgetState;
    use crate::{
        entities::{Journal, JournalPropertyChange},
        test_support::{local_datetime, render_snapshot},
    };

    fn create_journal(
        creator: String,
        updated_at: DateTime<Local>,
        properties: Vec<JournalPropertyChange>,
        comment: Option<String>,
    ) -> Journal {
        Journal {
            id: 1,
            creator,
            updated_at,
            properties,
            comment,
        }
    }

    #[test]
    fn snapshot_journals_list_mixed_entries() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![JournalPropertyChange {
            target: "担当者".to_string(),
            old: "(なし)".to_string(),
            new: "bob".to_string(),
        }];
        let comment = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &creator, &updated_at, &Some(comment.clone()));
        let journal = create_journal(creator, updated_at, properties, Some(comment));
        let journals = vec![JournalItemWidget::new(&journal, &state, true)];
        let line_count = JournalsListWidget::new(journals).line_count(24);
        let journals = vec![JournalItemWidget::new(&journal, &state, true)];

        render_snapshot(
            "journals_list_mixed_entries",
            24,
            line_count,
            JournalsListWidget::new(journals),
        );
    }

    #[test]
    fn snapshot_journals_list_clips_without_relayout_when_height_is_short() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![JournalPropertyChange {
            target: "担当者".to_string(),
            old: "(なし)".to_string(),
            new: "bob".to_string(),
        }];
        let comment = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &creator, &updated_at, &Some(comment.clone()));
        let journal = create_journal(creator, updated_at, properties, Some(comment));
        let journals = vec![JournalItemWidget::new(&journal, &state, true)];

        render_snapshot(
            "journals_list_clipped_height",
            24,
            5,
            JournalsListWidget::new(journals),
        );
    }

    #[test]
    fn line_count_journals_list_current_values() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![JournalPropertyChange {
            target: "担当者".to_string(),
            old: "(なし)".to_string(),
            new: "bob".to_string(),
        }];
        let comment = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut state = JournalItemWidgetState::new();
        state.update(24, &creator, &updated_at, &Some(comment.clone()));
        let journal = create_journal(creator, updated_at, properties, Some(comment));
        let journals = vec![JournalItemWidget::new(&journal, &state, false)];
        let widget = JournalsListWidget::new(journals);
        assert_eq!(widget.line_count(24), 9);
    }

    #[test]
    fn line_count_journals_list_changes_with_child_comment_height() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![JournalPropertyChange {
            target: "担当者".to_string(),
            old: "(なし)".to_string(),
            new: "bob".to_string(),
        }];
        let short_comment = "first paragraph with enough words to wrap".to_string();
        let long_comment =
            "first paragraph\n\nsecond paragraph with wrapping words that expands the comment"
                .to_string();

        let wide_short = {
            let mut state = JournalItemWidgetState::new();
            state.update(32, &creator, &updated_at, &Some(short_comment.clone()));
            let journal = create_journal(
                creator.clone(),
                updated_at,
                properties.clone(),
                Some(short_comment.clone()),
            );
            let journals = vec![JournalItemWidget::new(&journal, &state, false)];
            JournalsListWidget::new(journals).line_count(32)
        };

        let narrow_short = {
            let mut state = JournalItemWidgetState::new();
            state.update(18, &creator, &updated_at, &Some(short_comment.clone()));
            let journal = create_journal(
                creator.clone(),
                updated_at,
                properties.clone(),
                Some(short_comment.clone()),
            );
            let journals = vec![JournalItemWidget::new(&journal, &state, false)];
            JournalsListWidget::new(journals).line_count(18)
        };

        let narrow_long = {
            let mut state = JournalItemWidgetState::new();
            state.update(18, &creator, &updated_at, &Some(long_comment.clone()));
            let journal = create_journal(
                creator.clone(),
                updated_at,
                properties.clone(),
                Some(long_comment.clone()),
            );
            let journals = vec![JournalItemWidget::new(&journal, &state, false)];
            JournalsListWidget::new(journals).line_count(18)
        };

        assert!(wide_short < narrow_short);
        assert!(narrow_short < narrow_long);
    }

    #[test]
    fn line_count_property_only_increases_with_property_count() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let properties = vec![
            JournalPropertyChange {
                target: "ステータス".to_string(),
                old: "新規".to_string(),
                new: "進行中".to_string(),
            },
            JournalPropertyChange {
                target: "担当者".to_string(),
                old: "(なし)".to_string(),
                new: "bob".to_string(),
            },
        ];

        let journal = create_journal(creator, updated_at, properties, None);
        let state = JournalItemWidgetState::new();
        let widget = JournalItemWidget::new(&journal, &state, false);
        assert_eq!(widget.line_count(20), 6);
    }
}
