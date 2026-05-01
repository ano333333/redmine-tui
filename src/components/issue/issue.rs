use std::fs;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use yaml_rust::{Yaml, YamlLoader};

use crate::app::Store;
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
    pub fn parse_yaml() -> Self {
        let path = "datas/issue.yml";
        let yaml_raw = fs::read_to_string(path).expect(format!("failed to read {}", path).as_str());
        let yaml = YamlLoader::load_from_str(yaml_raw.as_str())
            .expect(format!("failed to parse {}", path).as_str());
        let yaml = &yaml[0];
        let id = yaml["id"].as_i64().expect("no id").try_into().unwrap_or(0);
        let title = yaml["title"].as_str().expect("no title").to_string();
        let creator = yaml["creator"].as_str().expect("no creator").to_string();
        let appended_at = yaml["appended_at"].as_str().expect("no appended_at");
        let appended_at = Self::parse_as_local(appended_at);
        let updated_at = yaml["updated_at"].as_str().expect("no updated_at");
        let updated_at = Self::parse_as_local(updated_at);
        let status = yaml["status"].as_str().expect("no status").to_string();
        let priority = yaml["priority"].as_str().expect("no priority").to_string();
        let person_in_charge = Self::parse_as_option_str(&yaml, "person_in_charge");
        let target_version = Self::parse_as_option_str(&yaml, "target_version");
        let start_date = Self::parse_as_option_datetime(&yaml, "due");
        let due = Self::parse_as_option_datetime(&yaml, "due");
        let progress = yaml["progress"]
            .as_i64()
            .expect("no progress")
            .try_into()
            .unwrap_or(0);
        let planned_hours = Self::parse_as_option_u16(&yaml, "planned_hours");
        let resolve_way = Self::parse_as_option_str(&yaml, "resolve_way");
        let component = yaml["component"]
            .as_str()
            .expect("no component")
            .to_string();
        let tags = Self::parse_as_string_array(&yaml, "tags");
        let body = yaml["body"].as_str().expect("no body").to_string();
        let relatives_path = "datas/relatives.yml".to_string();
        let articles_path = "datas/articles.yml".to_string();
        IssueComponent {
            id,
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
            relatives: RelativeIssueComponent::parse_yaml(&relatives_path),
            journals: JournalComponent::parse_yaml(&articles_path),
        }
    }
    fn create_header(&self) -> Vec<Paragraph> {
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

    fn create_status_table(&self) -> Vec<Paragraph> {
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

    fn create_body(&self) -> Vec<Paragraph> {
        vec![Paragraph::new(tui_markdown::from_str(&self.body)).wrap(Wrap { trim: true })]
    }

    fn parse_as_local(str: &str) -> DateTime<Local> {
        let naive = NaiveDate::parse_from_str(str, "%Y/%m/%d")
            .expect(format!("failed to parse naive datetime: {}", str).as_str());
        let naive_datetime = naive.and_hms_opt(0, 0, 0).unwrap();
        Local.from_local_datetime(&naive_datetime).single().unwrap()
    }
    fn parse_as_option_str(yaml: &Yaml, key: &str) -> Option<String> {
        if let Some(c) = yaml[key].as_str() {
            Some(c.to_string())
        } else {
            None
        }
    }
    fn parse_as_option_datetime(yaml: &Yaml, key: &str) -> Option<DateTime<Local>> {
        if let Some(c) = yaml[key].as_str() {
            Some(Self::parse_as_local(c))
        } else {
            None
        }
    }
    fn parse_as_option_u16(yaml: &Yaml, key: &str) -> Option<u16> {
        if let Some(c) = yaml[key].as_i64() {
            Some(c.try_into().unwrap_or(0))
        } else {
            None
        }
    }
    fn parse_as_string_array(yaml: &Yaml, key: &str) -> Vec<String> {
        if let Some(arr) = yaml[key].as_vec() {
            let mut res = Vec::<String>::new();
            for s in arr {
                res.push(s.as_str().unwrap().to_string());
            }
            res
        } else {
            Vec::<String>::new()
        }
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
