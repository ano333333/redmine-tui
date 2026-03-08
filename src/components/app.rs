use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Action, Dispatcher, Store};
use crate::components::issue::IssueComponent;
use crate::components::{Component, JournalComponent, RelativeIssueComponent};

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

    pub fn process_event(&self, event: Event) {}
}

impl Component for AppComponent {
    fn line_count(&self, width: u16) -> u16 {
        self.issue_component.line_count(width)
    }

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        self.issue_component.render(store, frame, area);
    }
}
