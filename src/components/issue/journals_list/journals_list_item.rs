use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;

use crate::app::Store;

use super::focus_events::FocusEvent;
use super::process_event_results::EventProcessResult;

pub trait JournalsListItem {
    fn get_id(&self) -> u16;
    fn update(&mut self, _: &Store, _: u16);
    fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer>;
    fn line_count(&self, store: &Store, width: u16) -> u16;
    fn focus_event(&mut self, store: &Store, event: FocusEvent, width: u16);
    fn process_event(
        &mut self,
        event: &Event,
        store: &Store,
        width: u16,
    ) -> Option<EventProcessResult>;
    fn get_cursor_position(&self) -> Position;
}
