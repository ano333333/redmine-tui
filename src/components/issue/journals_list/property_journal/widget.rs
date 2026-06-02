use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget};

#[derive(Clone)]
pub struct PropertyJournalWidget<'a> {
    creator: &'a String,
    target: &'a String,
    old: &'a String,
    new: &'a String,
    updated_at: &'a DateTime<Local>,
}

impl<'a> PropertyJournalWidget<'a> {
    pub fn new(
        creator: &'a String,
        target: &'a String,
        old: &'a String,
        new: &'a String,
        updated_at: &'a DateTime<Local>,
    ) -> Self {
        Self {
            creator,
            target,
            old,
            new,
            updated_at,
        }
    }

    pub fn line_count(&self, _: u16) -> u16 {
        4
    }

    fn create_paragraph(&self) -> Paragraph<'static> {
        let title = Line::from(vec![
            Span::from(self.creator.clone()).blue(),
            Span::from("が"),
            Span::from(self.updated_at.format("%Y/%m/%d").to_string()).blue(),
            Span::from("に更新"),
        ]);
        let body = Line::from(vec![
            Span::from("  ・ "),
            Span::from(self.target.clone()).bold(),
            Span::from(" を "),
            Span::from(self.old.clone()).italic(),
            Span::from(" から "),
            Span::from(self.new.clone()).italic(),
            Span::from(" に変更"),
        ])
        .gray();
        Paragraph::new(Text::from(vec![title, Line::from(""), body, Line::from("")]))
    }
}

impl Widget for PropertyJournalWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.create_paragraph().render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};

    #[test]
    fn snapshot_property_journal_basic() {
        let creator = "alice".to_string();
        let target = "ステータス".to_string();
        let old = "新規".to_string();
        let new = "進行中".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        render_snapshot(
            "property_journal_basic",
            32,
            4,
            PropertyJournalWidget::new(&creator, &target, &old, &new, &updated_at),
        );
    }

    #[test]
    fn line_count_property_journal_is_fixed_4() {
        let creator = "alice".to_string();
        let target = "ステータス".to_string();
        let old = "新規".to_string();
        let new = "進行中".to_string();
        let updated_at = local_datetime("2026-01-15T00:00:00+09:00");
        let widget = PropertyJournalWidget::new(&creator, &target, &old, &new, &updated_at);
        assert_eq!(widget.line_count(20), 4);
    }
}
