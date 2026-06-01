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
