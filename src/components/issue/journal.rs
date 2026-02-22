use std::fs;

use chrono::NaiveDate;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use yaml_rust::YamlLoader;

use crate::components::Component;

pub enum JournalComponent {
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

impl JournalComponent {
    pub fn parse_yaml(path: &String) -> Vec<JournalComponent> {
        let mut comps = Vec::<JournalComponent>::new();
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

impl Component for JournalComponent {
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

    fn render(&self, frame: &mut Frame, mut area: Rect) {
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
                area.y += 1;
                area.height = area.height.saturating_sub(1);
                frame.render_widget(hr, area);
                area.y += 2;
                area.height = area.height.saturating_sub(2);
                frame.render_widget(body, area);
            }
        }
    }
}
