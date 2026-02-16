use std::fs;
use std::io::Result;

use chrono::{DateTime, Local, NaiveDate};
use ratatui::Frame;
use ratatui::layout::{Offset, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use yaml_rust::YamlLoader;

use super::Component;

pub enum IssueJournalComponent {
    Property {
        creator: String,
        target: String,
        old: String,
        new: String,
        updated_at: NaiveDate,
    },
    Comment {
        creator: String,
        updated_at: NaiveDate,
        body: String,
    },
}

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
    pub journals: Vec<IssueJournalComponent>,
}

impl IssueComponent {
    fn create_paragraphs(&self) -> Vec<Paragraph> {
        let mut paragraphs = Vec::<Paragraph>::new();
        let mut lines = Vec::<Line>::from([]);
        let mut header = self.create_header();
        lines.append(&mut header);
        let mut status_table = self.create_status_table();
        lines.append(&mut status_table);
        paragraphs.push(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(Block::default()),
        );
        paragraphs.push(Paragraph::new("─".to_string().repeat(120)).style(Style::default().gray()));
        paragraphs
            .push(Paragraph::new(tui_markdown::from_str(&self.body)).wrap(Wrap { trim: true }));
        paragraphs
    }

    fn create_header(&self) -> Vec<Line> {
        vec![
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
        ]
    }

    fn create_status_table(&self) -> Vec<Line> {
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
        vec![
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
        ]
    }
}

impl Component for IssueComponent {
    fn line_count(&self, width: u16) -> u16 {
        let paragraphs = self.create_paragraphs();
        let paragraphs_line = paragraphs
            .iter()
            .fold(0, |acc, p| acc + p.line_count(width) as u16);
        let journals_line = self
            .journals
            .iter()
            .fold(0, |acc, comp| acc + comp.line_count(width));
        paragraphs_line + journals_line
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let mut line: i32 = 0;
        for p in self.create_paragraphs().iter() {
            frame.render_widget(p, area.offset(Offset { x: 0, y: line }));
            line += p.line_count(area.width) as i32;
        }
        for j in self.journals.iter() {
            j.render(frame, area.offset(Offset { x: 0, y: line }));
            line += j.line_count(area.width) as i32;
        }
    }
}

impl IssueJournalComponent {
    pub fn parse_yaml(path: &String) -> Vec<IssueJournalComponent> {
        let mut comps = Vec::<IssueJournalComponent>::new();
        let yaml_row = fs::read_to_string(path).expect(format!("failed to read {}", path).as_str());
        let yaml = YamlLoader::load_from_str(&yaml_row)
            .expect(format!("failed to parse {}", path).as_str());
        for doc in yaml {
            let journal_type = doc["type"].as_str().expect("no type");
            let creator = doc["creator"].as_str().expect("no creator").to_string();
            let updated_at = NaiveDate::parse_from_str(
                doc["updated_at"]
                    .as_str()
                    .expect("no updated_at in property type"),
                "%Y/%m/%d",
            )
            .expect("parse error of updated_at");
            match journal_type {
                "property" => {
                    let target = doc["target"]
                        .as_str()
                        .expect("no target in property type")
                        .to_string();
                    let old = doc["old"]
                        .as_str()
                        .expect("no old in property type")
                        .to_string();
                    let new = doc["new"]
                        .as_str()
                        .expect("no new in property type")
                        .to_string();
                    comps.push(Self::Property {
                        creator,
                        target,
                        old,
                        new,
                        updated_at,
                    })
                }
                "comment" => {
                    let body = doc["body"]
                        .as_str()
                        .expect("no body in comment type")
                        .to_string();
                    comps.push(Self::Comment {
                        creator,
                        updated_at,
                        body,
                    });
                }
                _ => {}
            }
        }
        comps
    }
}

impl Component for IssueJournalComponent {
    fn line_count(&self, width: u16) -> u16 {
        match self {
            Self::Property { .. } => 4,
            Self::Comment { body, .. } => {
                let body_text = tui_markdown::from_str(body);
                Paragraph::new(body_text)
                    .wrap(Wrap { trim: true })
                    .line_count(width) as u16
                    + 3
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        match self {
            Self::Property {
                creator,
                target,
                old,
                new,
                updated_at,
            } => {
                let title = Line::from(vec![
                    Span::from(creator).blue(),
                    Span::from("が"),
                    Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
                    Span::from("に更新"),
                ]);
                let hr = Line::from("─".to_string().repeat(120)).gray();
                let body = Line::from(vec![
                    Span::from("  ・ "),
                    Span::from(target).bold(),
                    Span::from(" を "),
                    Span::from(old).italic(),
                    Span::from(" から "),
                    Span::from(new).italic(),
                    Span::from(" に変更"),
                ])
                .gray();
                let margin = Line::from("");
                frame.render_widget(
                    Paragraph::new(Text::from(vec![title, hr, body, margin])),
                    area,
                );
            }
            Self::Comment {
                creator,
                updated_at,
                body,
            } => {
                let title = Line::from(vec![
                    Span::from(creator).blue(),
                    Span::from("が"),
                    Span::from(updated_at.format("%Y/%m/%d").to_string()).blue(),
                    Span::from("に更新"),
                ]);
                let hr = Line::from("─".to_string().repeat(120)).gray();
                let mut body = tui_markdown::from_str(body);
                body.lines.push(Line::from(""));
                frame.render_widget(title, area);
                frame.render_widget(hr, area.offset(Offset { x: 0, y: 1 }));
                frame.render_widget(body, area.offset(Offset { x: 0, y: 2 }));
            }
        }
    }
}
