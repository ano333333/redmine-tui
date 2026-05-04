use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::buffer::Buffer;
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
    id: u16,
    header: IssueHeaderComponent,
    property: IssuePropertyComponent,
    body: IssueBodyComponent,
    children_list: IssueChildrenListComponent,
    journals: Vec<JournalComponent>,
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
        if area.height > 0
            && let Some(buffer) = self.header.render(store, area.width, area.height)
        {
            render_buffer_to_frame(frame, &mut area, &buffer);
        }

        if area.height > 0
            && let Some(buffer) = self.property.render(store, area.width, area.height)
        {
            render_buffer_to_frame(frame, &mut area, &buffer);
        }

        if area.height > 0 {
            frame.render_widget(Hr::default(), area);
            area.y += 1;
            area.height -= 1;
        }

        if area.height > 0
            && let Some(buffer) = self.body.render(store, area.width, area.height)
        {
            render_buffer_to_frame(frame, &mut area, &buffer);
        }

        if area.height > 0 {
            frame.render_widget(Hr::default(), area);
            area.y += 1;
            area.height -= 1;
        }

        if area.height > 0
            && let Some(buffer) = self.children_list.render(store, area.width, area.height)
        {
            render_buffer_to_frame(frame, &mut area, &buffer);
        }

        if area.height > 0 {
            frame.render_widget(Hr::default(), area);
            area.y += 1;
            area.height -= 1;
        }

        for j in self.journals.iter() {
            if area.height > 0
                && let Some(buffer) = j.render(store, area.width, area.height)
            {
                render_buffer_to_frame(frame, &mut area, &buffer);
            }
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

fn render_buffer_to_frame(frame: &mut Frame, area: &mut Rect, buffer: &Buffer) {
    let width = area.width.min(buffer.area.width);
    let height = area.height.min(buffer.area.height);

    let frame_buffer = frame.buffer_mut();
    for y in 0..height {
        for x in 0..width {
            let Some(src_cell) = buffer.cell((x, y)).cloned() else {
                continue;
            };
            let dst_x = area.x + x;
            let dst_y = area.y + y;
            if let Some(dst_cell) = frame_buffer.cell_mut((dst_x, dst_y)) {
                *dst_cell = src_cell;
            }
        }
    }

    area.y += height;
    area.height -= height;
}
