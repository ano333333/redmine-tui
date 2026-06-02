use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::comment_journal::widget::CommentJournalWidget;
use super::property_journal::widget::PropertyJournalWidget;

pub enum JournalItemWidget<'a> {
    Comment(CommentJournalWidget<'a>),
    Property(PropertyJournalWidget<'a>),
}

pub struct JournalsListWidget<'a> {
    journals: &'a [JournalItemWidget<'a>],
}

impl<'a> JournalsListWidget<'a> {
    pub fn new(journals: &'a [JournalItemWidget<'a>]) -> Self {
        Self { journals }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        self.journals.iter().map(|journal| journal.line_count(width)).sum()
    }
}

impl Widget for JournalsListWidget<'_> {
    fn render(self, mut area: Rect, buf: &mut Buffer) {
        for journal in self.journals {
            let line_count = journal.line_count(area.width);
            if line_count == 0 || area.height == 0 {
                continue;
            }

            let item_height = line_count.min(area.height);
            let item_area = Rect::new(area.x, area.y, area.width, item_height);
            journal.render_ref(item_area, buf);

            area.y += item_height;
            area.height = area.height.saturating_sub(item_height);
        }
    }
}

impl Widget for JournalItemWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            JournalItemWidget::Comment(widget) => widget.clone().render(area, buf),
            JournalItemWidget::Property(widget) => widget.clone().render(area, buf),
        }
    }
}

impl JournalItemWidget<'_> {
    pub fn line_count(&self, width: u16) -> u16 {
        match self {
            JournalItemWidget::Comment(widget) => widget.line_count(width),
            JournalItemWidget::Property(widget) => widget.line_count(width),
        }
    }

    pub fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            JournalItemWidget::Comment(widget) => widget.clone().render(area, buf),
            JournalItemWidget::Property(widget) => widget.clone().render(area, buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        components::issue::journals_list::{
            comment_journal::widget::{CommentJournalWidget, CommentJournalWidgetState},
            property_journal::widget::PropertyJournalWidget,
        },
        test_support::{local_datetime, render_snapshot},
    };

    #[test]
    fn snapshot_journals_list_mixed_entries() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let target = "担当者".to_string();
        let old = "(なし)".to_string();
        let new = "bob".to_string();
        let comment = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut comment_state = CommentJournalWidgetState::new();
        comment_state.update(24, &creator, &updated_at, &comment);

        let property_widget = PropertyJournalWidget::new(&creator, &target, &old, &new, &updated_at);
        let comment_widget = CommentJournalWidget::new(&comment_state);
        let journals = [
            JournalItemWidget::Property(property_widget),
            JournalItemWidget::Comment(comment_widget),
        ];

        render_snapshot(
            "journals_list_mixed_entries",
            24,
            JournalsListWidget::new(&journals).line_count(24),
            JournalsListWidget::new(&journals),
        );
    }

    #[test]
    fn line_count_journals_list_current_values() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let target = "担当者".to_string();
        let old = "(なし)".to_string();
        let new = "bob".to_string();
        let comment = "first paragraph\n\nsecond paragraph with wrapping words".to_string();

        let mut comment_state = CommentJournalWidgetState::new();
        comment_state.update(24, &creator, &updated_at, &comment);

        let property_widget = PropertyJournalWidget::new(&creator, &target, &old, &new, &updated_at);
        let comment_widget = CommentJournalWidget::new(&comment_state);
        let journals = [
            JournalItemWidget::Property(property_widget),
            JournalItemWidget::Comment(comment_widget),
        ];
        let widget = JournalsListWidget::new(&journals);
        assert_eq!(widget.line_count(24), 10);
    }

    #[test]
    fn line_count_journals_list_changes_with_child_comment_height() {
        let creator = "alice".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let target = "担当者".to_string();
        let old = "(なし)".to_string();
        let new = "bob".to_string();
        let short_comment = "first paragraph with enough words to wrap".to_string();
        let long_comment =
            "first paragraph\n\nsecond paragraph with wrapping words that expands the comment"
                .to_string();

        let mut comment_state = CommentJournalWidgetState::new();
        let property_widget = PropertyJournalWidget::new(&creator, &target, &old, &new, &updated_at);

        comment_state.update(32, &creator, &updated_at, &short_comment);
        let wide_short = {
            let journals = [
                JournalItemWidget::Property(property_widget.clone()),
                JournalItemWidget::Comment(CommentJournalWidget::new(&comment_state)),
            ];
            JournalsListWidget::new(&journals).line_count(32)
        };

        comment_state.update(18, &creator, &updated_at, &short_comment);
        let narrow_short = {
            let journals = [
                JournalItemWidget::Property(property_widget.clone()),
                JournalItemWidget::Comment(CommentJournalWidget::new(&comment_state)),
            ];
            JournalsListWidget::new(&journals).line_count(18)
        };

        comment_state.update(18, &creator, &updated_at, &long_comment);
        let narrow_long = {
            let journals = [
                JournalItemWidget::Property(property_widget),
                JournalItemWidget::Comment(CommentJournalWidget::new(&comment_state)),
            ];
            JournalsListWidget::new(&journals).line_count(18)
        };

        assert!(wide_short < narrow_short);
        assert!(narrow_short < narrow_long);
    }
}
