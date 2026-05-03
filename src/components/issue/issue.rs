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
    pub relatives: Vec<RelativeIssueComponent>,
    pub journals: Vec<JournalComponent>,
}

impl IssueComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        let d = dispatcher.borrow();
        let issue = d.store().get_issue(issue_id);
        if issue.is_none() {
            panic!();
        }
        let issue = issue.unwrap();
        let relatives = issue
            .relative_ids
            .iter()
            .map(|i| RelativeIssueComponent::new(dispatcher.clone(), *i))
            .collect();
        let journals = issue
            .journal_ids
            .iter()
            .map(|i| JournalComponent::new(dispatcher.clone(), *i))
            .collect();
        IssueComponent {
            id: issue_id,
            title: issue.title.clone(),
            creator: issue.creator.clone(),
            appended_at: issue.appended_at,
            updated_at: issue.updated_at,
            status: issue.status.clone(),
            priority: issue.priority.clone(),
            person_in_charge: issue.person_in_charge.clone(),
            target_version: issue.target_version.clone(),
            start_date: issue.start_date,
            due: issue.due,
            progress: issue.progress,
            planned_hours: issue.planned_hours,
            resolve_way: issue.resolve_way.clone(),
            component: issue.component.clone(),
            tags: issue.tags.clone(),
            body: issue.body.clone(),
            relatives,
            journals,
        }
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
            .fold(0, |acc, comp| acc + comp.line_count(width));
        let journals_line = self
            .journals
            .iter()
            .fold(0, |acc, comp| acc + comp.line_count(width));
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
        let child_complete_num = self.relatives.iter().filter(|c| c.complete).count();
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
            j.render(store, frame, area);
            let l = j.line_count(area.width);
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
    }
}
