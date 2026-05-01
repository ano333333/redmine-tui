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

pub struct RelativeIssueComponent {
    pub id: u16,
    pub complete: bool,
    pub title: String,
    pub status: String,
    pub person_in_charge: Option<String>,
    pub start_date: Option<DateTime<Local>>,
    pub due: Option<DateTime<Local>>,
    pub progress: u16,
}

impl RelativeIssueComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        let dispatcher = dispatcher.borrow();
        let issue = dispatcher.store().get_issue(issue_id);
        if issue.is_none() {
            panic!();
        }
        let issue = issue.unwrap();
        RelativeIssueComponent {
            id: issue_id,
            complete: issue.status == "完了",
            title: issue.title.clone(),
            status: issue.status.clone(),
            person_in_charge: issue.person_in_charge.clone(),
            start_date: issue.start_date,
            due: issue.due,
            progress: issue.progress,
        }
    }
}

impl Component for RelativeIssueComponent {
    fn line_count(&self, _: u16) -> u16 {
        1
    }

    fn render(&self, _: &Store, frame: &mut Frame, area: Rect) {
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

        let id_style = if self.complete {
            Style::default().add_modifier(Modifier::CROSSED_OUT).gray()
        } else {
            Style::default().blue()
        };
        let id = Paragraph::new(Text::from(format!("#{}", self.id))).style(id_style);
        frame.render_widget(id, cols[0]);

        let title = Paragraph::new(Text::from(self.title.clone())).wrap(Wrap { trim: true });
        frame.render_widget(title, cols[2]);

        let status = Paragraph::new(Text::from(self.status.clone())).wrap(Wrap { trim: true });
        frame.render_widget(status, cols[4]);

        if let Some(s) = &self.person_in_charge {
            let person_in_change = Paragraph::new(Text::from(s.clone())).blue();
            frame.render_widget(person_in_change, cols[6]);
        }

        if let Some(t) = &self.start_date {
            let start_date = Paragraph::new(Text::from(t.format("%Y/%m/%d").to_string()));
            frame.render_widget(start_date, cols[8]);
        }

        if let Some(t) = &self.due {
            let due = Paragraph::new(Text::from(t.format("%Y/%m/%d").to_string()));
            frame.render_widget(due, cols[10]);
        }

        let progress = Paragraph::new(Text::from(format!("{:>3}%", self.progress.to_string())));
        frame.render_widget(progress, cols[12]);
    }
}
