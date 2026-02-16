use ratatui::Frame;
use ratatui::layout::Rect;

pub trait Component {
    fn render(&self, frame: &mut Frame, area: Rect);
    fn line_count(&self, width: u16) -> u16;
}
