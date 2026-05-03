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

pub struct IssueComponent<'a> {
    pub id: u16,
    pub relatives: Vec<RelativeIssueComponent<'a>>,
    pub journals: Vec<JournalComponent<'a>>,
    widgets: Option<IssueComponentWidgets<'a>>,
}

impl<'a> IssueComponent<'a> {
    pub fn new(_: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        IssueComponent {
            id: issue_id,
            relatives: vec![],
            journals: vec![],
            widgets: None,
        }
    }
}

impl<'a> Component for IssueComponent<'a> {
    fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        let issue = store.get_issue(self.id);
        match issue {
            None => {
                self.relatives = vec![];
                self.journals = vec![];
                self.widgets = None;
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
                self.widgets = Some(IssueComponentWidgets::new(
                    self.id,
                    &issue.title,
                    &issue.creator,
                    &issue.appended_at,
                    &issue.updated_at,
                    &issue.status,
                    &issue.priority,
                    &issue.person_in_charge,
                    &issue.target_version,
                    &issue.start_date,
                    &issue.due,
                    issue.progress,
                    issue.planned_hours,
                    &issue.resolve_way,
                    &issue.component,
                    &issue.tags,
                    &issue.body,
                    child_all_num,
                    child_complete_num,
                    child_imcomplete_num,
                ));
            }
        }
    }

    fn line_count(&self, width: u16) -> u16 {
        let widgets = match &self.widgets {
            Some(widgets) => widgets.line_count(width),
            None => 0,
        };
        let relatives_line = self
            .relatives
            .iter()
            .fold(0, |acc, comp| acc + comp.line_count(width));
        let journals_line = self
            .journals
            .iter()
            .fold(0, |acc, comp| acc + comp.line_count(width));
        widgets + relatives_line + 2 + journals_line
    }

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        match &self.widgets {
            None => {}
            Some(widgets) => {
                widgets.render(store, frame, &mut area);
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
    }
}

struct IssueComponentWidgets<'a> {
    pub header: Vec<Paragraph<'a>>,
    pub status_table: Vec<Paragraph<'a>>,
    body: String,
    pub childs_header: Text<'a>,
}

impl<'a> IssueComponentWidgets<'a> {
    pub fn new(
        id: u16,
        title: &String,
        creator: &String,
        appended_at: &DateTime<Local>,
        updated_at: &DateTime<Local>,
        status: &String,
        priority: &String,
        person_in_charge: &Option<String>,
        target_version: &Option<String>,
        start_date: &Option<DateTime<Local>>,
        due: &Option<DateTime<Local>>,
        progress: u16,
        planned_hours: Option<u16>,
        resolve_way: &Option<String>,
        component: &String,
        tags: &Vec<String>,
        body: &String,
        child_all_num: u16,
        child_complete_num: u16,
        child_imcomplete_num: u16,
    ) -> Self {
        Self {
            header: Self::create_header(id, title, creator, appended_at, updated_at),
            status_table: Self::create_status_table(
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
            ),
            body: body.clone(),
            childs_header: Self::create_childs_header(
                child_all_num,
                child_complete_num,
                child_imcomplete_num,
            ),
        }
    }

    pub fn line_count(&self, width: u16) -> u16 {
        let header_line = self
            .header
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        let status_line = self
            .status_table
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        let body_line = self
            .create_body()
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        header_line + status_line + 1 + body_line + 3
    }

    pub fn render(&self, _: &Store, frame: &mut Frame, area: &mut Rect) {
        for p in self.header.iter() {
            frame.render_widget(p, *area);
            let l = p.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        for p in self.status_table.iter() {
            frame.render_widget(p, *area);
            let l = p.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        frame.render_widget(Hr::default(), *area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);
        for p in self.create_body().iter() {
            frame.render_widget(p, *area);
            let l = p.line_count(area.width) as u16;
            area.y += l;
            area.height = area.height.saturating_sub(l);
        }
        frame.render_widget(Hr::default(), *area);
        area.y += 1;
        area.height = area.height.saturating_sub(1);
        frame.render_widget(&self.childs_header, *area);
        area.y += 2;
        area.height = area.height.saturating_sub(2);
    }

    fn create_header(
        id: u16,
        title: &String,
        creator: &String,
        appended_at: &DateTime<Local>,
        updated_at: &DateTime<Local>,
    ) -> Vec<Paragraph<'static>> {
        vec![Paragraph::new(vec![
            Line::from(format!("#{}", id)),
            Line::from(""),
            Line::from(format!("# {}", title.clone())).style(Style::default().bold()),
            Line::from("\n"),
            Line::from(vec![
                Span::from(creator.clone()).style(Style::default().blue()),
                Span::from("が"),
                Span::from(appended_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に追加. "),
                Span::from(updated_at.format("%Y/%m/%d").to_string())
                    .style(Style::default().blue()),
                Span::from("に更新."),
            ]),
            Line::from("\n\n"),
        ])]
    }

    fn create_status_table(
        status: &String,
        priority: &String,
        person_in_charge: &Option<String>,
        target_version: &Option<String>,
        start_date: &Option<DateTime<Local>>,
        due: &Option<DateTime<Local>>,
        progress: u16,
        planned_hours: Option<u16>,
        resolve_way: &Option<String>,
        component: &String,
        tags: &Vec<String>,
    ) -> Vec<Paragraph<'static>> {
        let person = match person_in_charge {
            Some(s) => s.clone(),
            None => "-".to_string(),
        };
        let target_version = match target_version {
            Some(s) => s.clone(),
            None => "-".to_string(),
        };
        fn datetime_opt_to_str(date_opt: &Option<DateTime<Local>>) -> String {
            match date_opt {
                Some(d) => d.format("%Y/%m/%d").to_string(),
                None => "-".to_string(),
            }
        }
        let planned_hours = match planned_hours {
            Some(p) => p.to_string(),
            None => "".to_string(),
        };
        let resolve_way = match resolve_way {
            Some(r) => r.clone(),
            None => "-".to_string(),
        };
        vec![Paragraph::new(vec![
            Line::from(vec![
                Span::from("ステータス          ").style(Style::default().blue()),
                Span::from(status.clone()),
            ]),
            Line::from(format!("優先度              {}", priority.clone())),
            Line::from(format!("担当者              {}", person)).style(Style::default().blue()),
            Line::from(format!("対象バージョン      {}", target_version)),
            Line::from(format!(
                "開始日              {}",
                datetime_opt_to_str(start_date)
            )),
            Line::from(format!("期日                {}", datetime_opt_to_str(due))),
            Line::from(vec![
                Span::from("進捗率              ").style(Style::default().blue()),
                Span::from(progress.to_string()),
            ]),
            Line::from(format!("予定工数            {}", planned_hours)),
            Line::from(format!("解決方法            {}", resolve_way)),
            Line::from(format!("コンポーネント      {}", component)),
            Line::from(format!("Tags                {}", tags.concat())),
        ])]
    }

    fn create_body(&self) -> Vec<Paragraph<'_>> {
        vec![Paragraph::new(tui_markdown::from_str(&self.body)).wrap(Wrap { trim: true })]
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
