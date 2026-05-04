use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};

use crate::AppContainer;
use crate::app::{Dispatcher, Store};
use crate::widgets::Hr;

use super::body::IssueBodyComponent;
use super::header::IssueHeaderComponent;
use super::journal::JournalComponent;
use super::property::IssuePropertyComponent;
use super::relative::RelativeIssueComponent;

pub struct IssueDetailComponent {
    pub id: u16,
    header: IssueHeaderComponent,
    property: IssuePropertyComponent,
    body: IssueBodyComponent,
    pub relatives: Vec<RelativeIssueComponent>,
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
                relatives: vec![],
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
                self.relatives = vec![];
                self.journals = vec![];
            }
            Some(issue) => {
                // FIXME: 差分更新
                self.relatives = issue
                    .relative_ids
                    .iter()
                    .map(|i| RelativeIssueComponent::new(dispatcher.clone(), *i))
                    .collect();
                self.relatives
                    .iter_mut()
                    .for_each(|relative| relative.update(dispatcher.clone(), store));
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
        let issue = store.get_issue(self.id);
        if let Some(issue) = issue {
            let child_all_num = issue.relative_ids.len() as u16;
            let child_complete_num = issue
                .relative_ids
                .iter()
                .map(|id| store.get_issue(*id))
                .filter(|issue| {
                    if let Some(issue) = issue {
                        issue.status == "完了"
                    } else {
                        false
                    }
                })
                .count() as u16;
            let child_imcomplete_num = child_all_num - child_complete_num;
            self.header.render(store, frame, &mut area);
            self.property.render(store, frame, &mut area);

            frame.render_widget(Hr::default(), area);
            area.y += 1;
            area.height = area.height.saturating_sub(1);

            self.body.render(store, frame, &mut area);

            frame.render_widget(Hr::default(), area);
            area.y += 1;
            area.height = area.height.saturating_sub(1);

            let widgets =
                IssueComponentWidgets::new(child_all_num, child_complete_num, child_imcomplete_num);
            widgets.render(store, frame, &mut area);
            for c in self.relatives.iter() {
                c.render(store, frame, area);
                area.y += 1;
                area.height = area.height.saturating_sub(1);
            }
            area.y += 1;
            area.height = area.height.saturating_sub(1);
            frame.render_widget(Hr::default(), area);
            area.y += 1;
            area.height = area.height.saturating_sub(1);
            for j in self.journals.iter() {
                j.render(store, frame, area);
                let l = j.line_count(store, area.width);
                area.y += l;
                area.height = area.height.saturating_sub(l);
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

struct IssueComponentWidgets {
    pub childs_header: Text<'static>,
}

impl IssueComponentWidgets {
    pub fn new(child_all_num: u16, child_complete_num: u16, child_imcomplete_num: u16) -> Self {
        Self {
            childs_header: Self::create_childs_header(
                child_all_num,
                child_complete_num,
                child_imcomplete_num,
            ),
        }
    }

    pub fn render(&self, _: &Store, frame: &mut Frame, area: &mut Rect) {
        frame.render_widget(&self.childs_header, *area);
        area.y += 2;
        area.height = area.height.saturating_sub(2);
    }

    fn create_childs_header(
        child_all_num: u16,
        child_complete_num: u16,
        child_imcomplete_num: u16,
    ) -> Text<'static> {
        let child_header_title = Line::from(vec![
            Span::from("子チケット").bold(),
            Span::from(" "),
            Span::from(format!(
                "{} ({}件未完了 - {}件完了)",
                child_all_num, child_complete_num, child_imcomplete_num
            )),
        ]);
        Text::from(vec![child_header_title, Line::from("")])
    }
}
