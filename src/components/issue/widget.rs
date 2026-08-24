use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Text},
    widgets::{Paragraph, Widget},
};

use super::detail::IssueDetailWidget;

pub(crate) enum IssueWidget<'a> {
    Fetching,
    FetchFailed { message: &'a str },
    Detail(IssueDetailWidget<'a>),
}

impl<'a> IssueWidget<'a> {
    pub(crate) fn fetching() -> Self {
        Self::Fetching
    }
    pub(crate) fn fetch_failed(message: &'a str) -> Self {
        Self::FetchFailed { message }
    }
    pub(crate) fn detail(widget: IssueDetailWidget<'a>) -> Self {
        Self::Detail(widget)
    }
}

impl Widget for IssueWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::Fetching => Paragraph::new("表示中です").render(area, buf),
            Self::FetchFailed { message } => Paragraph::new(Text::from(vec![
                Line::from(message),
                Line::from("r で再試行"),
            ]))
            .render(area, buf),
            Self::Detail(widget) => widget.render(area, buf),
        }
    }
}
