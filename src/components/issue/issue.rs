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
    render_offset_y: u16,
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
                render_offset_y: 0,
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

        if self.cursor_position.x >= self.width {
            self.cursor_position.x = self.width - 1;
        }
        let line_count = self.header.line_count(store)
            + self.property.line_count(store, self.width)
            + 1
            + self.body.line_count(store, self.width)
            + 1
            + self.children_list.line_count(store)
            + 1
            + self.journals.iter().fold(0, |acc, journal| {
                acc + journal.line_count(store, self.width)
            });
        if self.cursor_position.y >= line_count {
            self.cursor_position.y = line_count - 1;
        }

        self.update_offset_y(store, self.height);
    }

    pub fn render(&self, store: &Store, frame: &mut Frame, mut frame_area: Rect) {
        let mut line_count_sum: u16 = 0;
        let height = frame_area.height;
        let width = frame_area.width;
        let offset_y = self.render_offset_y;

        let line_count = self.header.line_count(store);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) = self
                .header
                .render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count = self.property.line_count(store, width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) = self
                .property
                .render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            frame.render_widget(Hr::default(), frame_area);
            frame_area.y += 1;
            frame_area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.body.line_count(store, width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) = self.body.render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            frame.render_widget(Hr::default(), frame_area);
            frame_area.y += 1;
            frame_area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.children_list.line_count(store);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) =
                self.children_list
                    .render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            frame.render_widget(Hr::default(), frame_area);
            frame_area.y += 1;
            frame_area.height -= 1;
        }
        line_count_sum += line_count;

        for j in self.journals.iter() {
            let line_count = j.line_count(store, width);
            if line_count_sum + line_count >= offset_y
                && line_count_sum < offset_y + height
                && frame_area.height > 0
                && let Some(buffer) = j.render(store, frame_area.width, frame_area.height)
            {
                let buffer_area = Rect::new(
                    0,
                    offset_y.saturating_sub(line_count_sum),
                    buffer.area.width,
                    line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
                );
                render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
            }
            line_count_sum += line_count;
        }

        AppContainer::set_cursor_position(
            frame,
            Position {
                x: self.cursor_position.x,
                y: self.cursor_position.y.saturating_sub(self.render_offset_y),
            },
        );
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
                }
                KeyCode::Char('k') => {
                    if self.cursor_position.y > 0 {
                        self.cursor_position.y -= 1;
                    }
                }
                KeyCode::Char('j') => {
                    self.cursor_position.y += 1;
                }
                _ => {}
            }
        } else if let Event::Resize(rows, cols) = event {
            self.width = rows;
            self.height = cols;
        }
    }

    fn update_offset_y(&mut self, store: &Store, area_height: u16) {
        let cursor_y: u16 = self.cursor_position.y;

        if cursor_y < self.render_offset_y {
            self.render_offset_y = cursor_y;
        }

        if cursor_y >= self.render_offset_y + area_height {
            self.render_offset_y = (cursor_y + 1).saturating_sub(area_height);
        }
    }
}

/// BufferをFrame先頭にコピーし、コピー先の書き込んだ領域を切り詰める
///
/// # Arguments
///
/// * `frame` - コピー先のFrame
/// * `frame_area` - `frame`の領域
/// * `buffer` - コピー元のBuffer
/// * `buffer_area` - `buffer`の領域
fn render_buffer_to_frame(
    frame: &mut Frame,
    frame_area: &mut Rect,
    buffer: &Buffer,
    buffer_area: Rect,
) {
    let width = frame_area.width.min(buffer_area.width);
    let height = frame_area.height.min(buffer_area.height);

    let frame_buffer = frame.buffer_mut();
    for y in 0..height {
        for x in 0..width {
            let Some(src_cell) = buffer.cell((buffer_area.x + x, buffer_area.y + y)).cloned()
            else {
                continue;
            };
            let dst_x = frame_area.x + x;
            let dst_y = frame_area.y + y;
            if let Some(dst_cell) = frame_buffer.cell_mut((dst_x, dst_y)) {
                *dst_cell = src_cell;
            }
        }
    }

    frame_area.y += height;
    frame_area.height -= height;
}
