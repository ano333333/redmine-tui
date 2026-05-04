use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Dispatcher, Store};
use crate::components::issue::IssueComponent;

pub struct AppComponent {
    issue_component: IssueComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
}

impl AppComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        AppComponent {
            issue_component: IssueComponent::new(dispatcher.clone(), 3),
            dispatcher,
        }
    }
    pub fn process_event(&mut self, event: Event) {
        self.issue_component.process_event(
            event,
            self.dispatcher.clone(),
            self.dispatcher.borrow().store(),
        );
    }
    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        self.issue_component.update(dispatcher, store);
    }

    pub fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        self.issue_component.render(store, frame, area);
    }
}
