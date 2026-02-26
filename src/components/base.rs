use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::Store;

pub trait Component {
    fn render(&self, store: &Store, frame: &mut Frame, area: Rect);
    fn line_count(&self, width: u16) -> u16;
}
