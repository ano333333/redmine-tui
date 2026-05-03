use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Dispatcher, Store};
use crate::components::Component;
use crate::components::issue::IssueComponent;

pub struct AppComponent<'a> {
    issue_component: IssueComponent<'a>,
    dispatcher: Rc<RefCell<Dispatcher>>,
}

impl<'a> AppComponent<'a> {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        AppComponent {
            issue_component: IssueComponent::new(dispatcher.clone(), 3),
            dispatcher,
        }
    }

    pub fn process_event(&self, _event: Event) {}
}

impl<'a> Component for AppComponent<'a> {
    fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        self.issue_component.update(dispatcher, store);
    }

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        self.issue_component.render(store, frame, area);
    }
}
