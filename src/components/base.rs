use std::cell::RefCell;
use std::rc::Rc;

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Dispatcher, Store};

pub trait Component {
    fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store);
    fn render(&self, store: &Store, frame: &mut Frame, area: Rect);
    fn line_count(&self, width: u16) -> u16;
}
