use std::cell::RefCell;
use std::rc::Rc;

use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{Dispatcher, Store};
use crate::components::Component;
use crate::entities::Issue;
use crate::widgets::Hr;

use super::journal::JournalComponent;
use super::relative::RelativeIssueComponent;

pub struct IssueComponent {
    pub id: u16,
    pub title: String,
    pub creator: String,
    pub appended_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub status: String,
    pub priority: String,
    pub person_in_charge: Option<String>,
    pub target_version: Option<String>,
    pub start_date: Option<DateTime<Local>>,
    pub due: Option<DateTime<Local>>,
    pub progress: u16,
    pub planned_hours: Option<u16>,
    pub resolve_way: Option<String>,
    pub component: String,
    pub tags: Vec<String>,
    pub body: String,
    pub relatives: Vec<Rc<RefCell<RelativeIssueComponent>>>,
    pub journals: Vec<Rc<RefCell<JournalComponent>>>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    observer_id: u16,
}

impl IssueComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Rc<RefCell<Self>> {
        let (
            observer_id,
            title,
            creator,
            appended_at,
            updated_at,
            status,
            priority,
            person_in_charge,
            target_version,
            start_date,
            due,
            progress,
            planned_hours,
            resolve_way,
            component,
            tags,
            body,
            relative_ids,
            journal_ids,
        ) = {
            let mut d = dispatcher.borrow_mut();
            let observer_id = d.issue_observer_id();
            let issue = d.store().get_issue(issue_id);
            if issue.is_none() {
                panic!();
            }
            let issue = issue.unwrap();
            (
                observer_id,
                issue.title.clone(),
                issue.creator.clone(),
                issue.appended_at,
                issue.updated_at,
                issue.status.clone(),
                issue.priority.clone(),
                issue.person_in_charge.clone(),
                issue.target_version.clone(),
                issue.start_date,
                issue.due,
                issue.progress,
                issue.planned_hours,
                issue.resolve_way.clone(),
                issue.component.clone(),
                issue.tags.clone(),
                issue.body.clone(),
                issue.relative_ids.clone(),
                issue.journal_ids.clone(),
            )
        };
        let relatives = relative_ids
            .iter()
            .map(|i| RelativeIssueComponent::new(dispatcher.clone(), *i))
            .collect();
        let journals = journal_ids
            .iter()
            .map(|i| JournalComponent::new(dispatcher.clone(), *i))
            .collect();
        let rc = Rc::new(RefCell::new(IssueComponent {
            id: issue_id,
            title,
            creator,
            appended_at,
            updated_at,
            status,
            priority,
            person_in_charge,
            target_version,
            start_date,
            due,
            progress,
            planned_hours,
            resolve_way,
            component,
            tags,
            body,
            relatives,
            journals,
            dispatcher: dispatcher.clone(),
            observer_id,
        }));
        let weak = Rc::downgrade(&rc);
        let mut d = dispatcher.borrow_mut();
        d.dispatch(crate::app::Action::AppendIssueObserver {
            issue_id,
            observer_id,
            observer: Box::new(move |issue| {
                if let Some(comp) = weak.upgrade() {
                    comp.borrow_mut().update(issue);
                }
            }),
        });
        rc
    }
    fn update(&mut self, issue: &Issue) {
        self.title = issue.title.clone();
        self.creator = issue.creator.clone();
        self.appended_at = issue.appended_at;
        self.updated_at = issue.updated_at;
        self.status = issue.status.clone();
        self.priority = issue.priority.clone();
        self.person_in_charge = issue.person_in_charge.clone();
        self.target_version = issue.target_version.clone();
        self.start_date = issue.start_date;
        self.due = issue.due;
        self.progress = issue.progress;
        self.planned_hours = issue.planned_hours;
        self.resolve_way = issue.resolve_way.clone();
        self.component = issue.component.clone();
        self.tags = issue.tags.clone();
        self.body = issue.body.clone();
        self.relatives = issue
            .relative_ids
            .iter()
            .map(|i| RelativeIssueComponent::new(self.dispatcher.clone(), *i))
            .collect();
        self.journals = issue
            .journal_ids
            .iter()
            .map(|i| JournalComponent::new(self.dispatcher.clone(), *i))
            .collect();
    }
    fn create_header(&self) -> Vec<Paragraph<'_>> {
        vec![Paragraph::new(vec![
            Line::from(format!("#{}", self.id)),
            Line::from(""),
            Line::from(format!("# {}", self.title)).style(Style::default().bold()),
            Line::from("\n"),
            Line::from(vec![
                Span::from(self.creator.clone()).style(Style::default().blue()),
                Span::from("が"),
                Span::from(self.appended_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に追加. "),
                Span::from(self.updated_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に更新."),
            ]),
            Line::from("\n\n"),
        ])]
    }

    fn create_status_table(&self) -> Vec<Paragraph<'_>> {
        let person = match &self.person_in_charge {
            Some(s) => s.clone(),
            None => "-".to_string(),
        };
        let target_version = match &self.target_version {
            Some(s) => s.clone(),
            None => "-".to_string(),
        };
        fn datetime_opt_to_str(date_opt: &Option<DateTime<Local>>) -> String {
            match date_opt {
                Some(d) => d.format("%Y/%m/%d").to_string(),
                None => "-".to_string(),
            }
        }
        let planned_hours = match self.planned_hours {
            Some(p) => p.to_string(),
            None => "".to_string(),
        };
        let resolve_way = match &self.resolve_way {
            Some(r) => r.clone(),
            None => "-".to_string(),
        };
        vec![Paragraph::new(vec![
            Line::from(vec![
                Span::from("ステータス          ").style(Style::default().blue()),
                Span::from(&self.status),
            ]),
            Line::from(format!("優先度              {}", &self.priority)),
            Line::from(format!("担当者              {}", person)).style(Style::default().blue()),
            Line::from(format!("対象バージョン      {}", target_version)),
            Line::from(format!(
                "開始日              {}",
                datetime_opt_to_str(&self.start_date)
            )),
            Line::from(format!(
                "期日                {}",
                datetime_opt_to_str(&self.due)
            )),
            Line::from(vec![
                Span::from("進捗率              ").style(Style::default().blue()),
                Span::from(self.progress.to_string()),
            ]),
            Line::from(format!("予定工数            {}", planned_hours)),
            Line::from(format!("解決方法            {}", resolve_way)),
            Line::from(format!("コンポーネント      {}", &self.component)),
            Line::from(format!("Tags                {}", self.tags.concat())),
        ])]
    }

    fn create_body(&self) -> Vec<Paragraph<'_>> {
        vec![Paragraph::new(tui_markdown::from_str(&self.body)).wrap(Wrap { trim: true })]
    }
}

impl Drop for IssueComponent {
    fn drop(&mut self) {
        let mut dispatcher = self.dispatcher.borrow_mut();
        dispatcher.dispatch(crate::app::Action::RemoveIssueObserver {
            issue_id: self.id,
            observer_id: self.observer_id,
        });
    }
}

impl Component for IssueComponent {
    fn line_count(&self, width: u16) -> u16 {
        let header_line = self
            .create_header()
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        let status_line = self
            .create_status_table()
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        let body_line = self
            .create_body()
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        let relatives_line = self
            .relatives
            .iter()
            .fold(0, |acc, comp| acc + comp.borrow().line_count(width));
        let journals_line = self
            .journals
            .iter()
            .fold(0, |acc, comp| acc + comp.borrow().line_count(width));
        header_line + status_line + 1 + body_line + 3 + relatives_line + 2 + journals_line
    }

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        for p in self.create_header().iter() {
            frame.render_widget(p, area);
            let l = p.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        for p in self.create_status_table().iter() {
            frame.render_widget(p, area);
            let l = p.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        frame.render_widget(Hr::default(), area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);
        for p in self.create_body().iter() {
            frame.render_widget(p, area);
            let l = p.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        let child_all_num = self.relatives.len();
        let child_complete_num = self
            .relatives
            .iter()
            .filter(|c| c.borrow().complete)
            .count();
        let child_imcomplete_num = child_all_num - child_complete_num;
        let child_header_title = Line::from(vec![
            Span::from("子チケット").bold(),
            Span::from(" "),
            Span::from(format!(
                "{} ({}件未完了 - {}件完了)",
                child_all_num, child_complete_num, child_imcomplete_num
            )),
        ]);
        frame.render_widget(Hr::default(), area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);
        let childs_header = Text::from(vec![child_header_title, Line::from("")]);
        frame.render_widget(childs_header, area);
        area.y += 2;
        area.height = area.height.saturating_sub(2);
        for c in self.relatives.iter() {
            let c = c.borrow();
            c.render(store, frame, area);
            let l = c.line_count(area.width);
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        area.y += 1;
        area.height = area.height.saturating_sub(1);
        frame.render_widget(Hr::default(), area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);
        for j in self.journals.iter() {
            let j = j.borrow();
            j.render(store, frame, area);
            let l = j.line_count(area.width);
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
    }
}
