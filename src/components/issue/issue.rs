use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use crate::AppContainer;
use crate::app::{Dispatcher, Store};
use crate::widgets::Hr;

use super::body::IssueBodyComponent;
use super::children_list::IssueChildrenListComponent;
use super::header::IssueHeaderComponent;
use super::journal::JournalComponent;
use super::property::IssuePropertyComponent;

pub struct IssueDetailComponent {
    pub id: u16,
    header: IssueHeaderComponent,
    property: IssuePropertyComponent,
    body: IssueBodyComponent,
    children_list: IssueChildrenListComponent,
    pub journals: Vec<JournalComponent>,
    cursor_position: Position,
    width: u16,
    height: u16,
}

impl IssueDetailComponent {
    pub fn new(_: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        let size = AppContainer::size();
        match size {
            Ok((width, height)) => IssueDetailComponent {
                id: issue_id,
                header: IssueHeaderComponent::new(issue_id),
                property: IssuePropertyComponent::new(issue_id),
                body: IssueBodyComponent::new(issue_id),
                children_list: IssueChildrenListComponent::new(issue_id),
                journals: vec![],
                cursor_position: Default::default(),
                width,
                height,
            },
            Err(_) => panic!(),
        }
    }

    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        let issue = store.get_issue(self.id);
        match issue {
            None => {
                self.journals = vec![];
            }
            Some(issue) => {
                // FIXME: 差分更新
                self.journals = issue
                    .journal_ids
                    .iter()
                    .map(|i| JournalComponent::new(dispatcher.clone(), *i))
                    .collect();
                self.journals
                    .iter_mut()
                    .for_each(|journal| journal.update(dispatcher.clone(), store));
            }
        }
    }

    pub fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        self.header.render(store, frame, &mut area);
        self.property.render(store, frame, &mut area);

        frame.render_widget(Hr::default(), area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);

        self.body.render(store, frame, &mut area);

        frame.render_widget(Hr::default(), area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);

        self.children_list.render(store, frame, &mut area);

        frame.render_widget(Hr::default(), area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);

        for j in self.journals.iter() {
            j.render(store, frame, area);
            let l = j.line_count(store, area.width);
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }

        AppContainer::set_cursor_position(frame, self.cursor_position);
    }

    pub fn process_event(
        &mut self,
        event: crossterm::event::Event,
        _: Rc<RefCell<Dispatcher>>,
        _: &Store,
    ) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('h') => {
                    if self.cursor_position.x > 0 {
                        self.cursor_position.x -= 1;
                    }
                }
                KeyCode::Char('l') => {
                    self.cursor_position.x += 1;
                    if self.cursor_position.x >= self.width {
                        self.cursor_position.x = self.width - 1;
                    }
                }
                KeyCode::Char('k') => {
                    if self.cursor_position.y > 0 {
                        self.cursor_position.y -= 1;
                    }
                }
                KeyCode::Char('j') => {
                    self.cursor_position.y += 1;
                    if self.cursor_position.y >= self.height {
                        self.cursor_position.y = self.height - 1;
                    }
                }
                _ => {}
            }
        } else if let Event::Resize(rows, cols) = event {
            self.width = rows;
            self.height = cols;
            if self.cursor_position.x >= rows {
                self.cursor_position.x = rows - 1;
            }
            if self.cursor_position.y >= cols {
                self.cursor_position.y = cols - 1;
            }
        }
    }
}
