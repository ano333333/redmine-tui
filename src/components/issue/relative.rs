use std::cell::RefCell;
use std::rc::Rc;

use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{Dispatcher, Store};
use crate::components::Component;

pub struct RelativeIssueComponent<'a> {
    pub id: u16,
    widgets: Option<RelativeIssueComponentWidgets<'a>>,
}

impl RelativeIssueComponent<'_> {
    pub fn new(_: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        RelativeIssueComponent {
            id: issue_id,
            widgets: None,
        }
    }
}

impl Component for RelativeIssueComponent<'_> {
    fn update(&mut self, _: Rc<RefCell<Dispatcher>>, store: &Store) {
        let issue = store.get_issue(self.id);
        match issue {
            Some(issue) => {
                // FIXME: 差分更新
                self.widgets = Some(RelativeIssueComponentWidgets::new(
                    issue.id,
                    &issue.status,
                    &issue.title,
                    &issue.person_in_charge,
                    &issue.start_date,
                    &issue.due,
                    issue.progress,
                ));
            }
            None => {
                self.widgets = None;
            }
        }
    }

    fn line_count(&self, _: u16) -> u16 {
        1
    }

    fn render(&self, _: &Store, frame: &mut Frame, area: Rect) {
        match &self.widgets {
            Some(widgets) => {
                let row = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Max(1)])
                    .split(area)[0];
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(7), // ID
                        Constraint::Length(1),
                        Constraint::Fill(1), // title
                        Constraint::Length(1),
                        Constraint::Length(7), // status
                        Constraint::Length(1),
                        Constraint::Length(12), // person_in_charge
                        Constraint::Length(1),
                        Constraint::Length(10), // created_at
                        Constraint::Length(1),
                        Constraint::Length(10), // due
                        Constraint::Length(1),
                        Constraint::Length(4), // progress
                    ])
                    .split(row);

                frame.render_widget(&widgets.id, cols[0]);
                frame.render_widget(&widgets.title, cols[2]);
                frame.render_widget(&widgets.status, cols[4]);
                frame.render_widget(&widgets.person_in_charge, cols[6]);
                frame.render_widget(&widgets.start_date, cols[8]);
                frame.render_widget(&widgets.due, cols[10]);
                frame.render_widget(&widgets.progress, cols[12]);
            }
            _ => {}
        }
    }
}

struct RelativeIssueComponentWidgets<'a> {
    pub id: Paragraph<'a>,
    pub title: Paragraph<'a>,
    pub status: Paragraph<'a>,
    pub person_in_charge: Paragraph<'a>,
    pub start_date: Paragraph<'a>,
    pub due: Paragraph<'a>,
    pub progress: Paragraph<'a>,
}
impl RelativeIssueComponentWidgets<'_> {
    pub fn new(
        id: u16,
        status: &String,
        title: &String,
        person_in_charge: &Option<String>,
        start_date: &Option<DateTime<Local>>,
        due: &Option<DateTime<Local>>,
        progress: u16,
    ) -> Self {
        RelativeIssueComponentWidgets {
            id: Self::create_id_widget(id, status),
            title: Self::create_title_widget(title),
            status: Self::create_status_widget(status),
            person_in_charge: Self::create_person_in_charge_widgte(person_in_charge),
            start_date: Self::create_start_date_widget(start_date),
            due: Self::create_due_widget(due),
            progress: Self::create_progress_widget(progress),
        }
    }
    fn create_id_widget(id: u16, status: &String) -> Paragraph<'static> {
        let id_style = if status == "完了" {
            Style::default().add_modifier(Modifier::CROSSED_OUT).gray()
        } else {
            Style::default().blue()
        };
        Paragraph::new(Text::from(format!("#{}", id))).style(id_style)
    }
    fn create_title_widget(title: &String) -> Paragraph<'static> {
        Paragraph::new(Text::from(title.clone())).wrap(Wrap { trim: true })
    }
    fn create_status_widget(status: &String) -> Paragraph<'static> {
        Paragraph::new(Text::from(status.clone())).wrap(Wrap { trim: true })
    }
    fn create_person_in_charge_widgte(person_in_charge: &Option<String>) -> Paragraph<'static> {
        match &person_in_charge {
            Some(p) => Paragraph::new(Text::from(p.clone())).blue(),
            None => Paragraph::new(Text::from("(なし)")).gray(),
        }
    }
    fn create_start_date_widget(start_date: &Option<DateTime<Local>>) -> Paragraph<'static> {
        match &start_date {
            Some(date) => Paragraph::new(Text::from(date.format("%Y/%m/%d").to_string())),
            None => Paragraph::new(Text::from("----/--/--".to_string())).gray(),
        }
    }
    fn create_due_widget(due: &Option<DateTime<Local>>) -> Paragraph<'static> {
        match &due {
            Some(due) => Paragraph::new(Text::from(due.format("%Y/%m/%d").to_string())),
            None => Paragraph::new(Text::from("----/--/--".to_string())).gray(),
        }
    }
    fn create_progress_widget(progress: u16) -> Paragraph<'static> {
        Paragraph::new(Text::from(format!("{:>3}%", progress.to_string())))
    }
}
