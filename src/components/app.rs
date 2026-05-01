use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Dispatcher, Store};
use crate::components::Component;
use crate::components::issue::IssueComponent;

pub struct AppComponent {
    issue_component: IssueComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
}

impl AppComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        AppComponent {
            issue_component: IssueComponent::parse_yaml(),
            dispatcher,
        }
    }

    pub fn process_event(&self, _event: Event) {}
}

impl Component for AppComponent {
    fn line_count(&self, width: u16) -> u16 {
        self.issue_component.line_count(width)
    }

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        self.issue_component.render(store, frame, area);
    }
}
