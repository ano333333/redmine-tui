use std::fs;

use chrono::NaiveDate;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Wrap};
use yaml_rust::YamlLoader;

use crate::components::Component;

pub struct RelativeIssueComponent {
    pub id: u16,
    pub complete: bool,
    pub title: String,
    pub status: String,
    pub person_in_charge: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub progress: u16,
}

impl RelativeIssueComponent {
    pub fn parse_yaml(path: &String) -> Vec<RelativeIssueComponent> {
        let yaml_row = fs::read_to_string(path).expect(format!("failed to read {}", path).as_str());
        let yaml = YamlLoader::load_from_str(&yaml_row)
            .expect(format!("failed to parse {}", path).as_str());
        let mut comps = Vec::<RelativeIssueComponent>::new();
        for doc in yaml {
            let id = doc["id"].as_i64().expect("no id") as u16;
            let id_inactive = doc["complete"].as_bool().expect("no complete");
            let title = doc["title"].as_str().expect("no title").to_string();
            let status = doc["status"].as_str().expect("no status").to_string();
            let mut person_in_charge: Option<String> = None;
            if let Some(s) = doc["person_in_charge"].as_str() {
                person_in_charge = Some(s.to_string());
            }
            let mut start_date: Option<NaiveDate> = None;
            if let Some(d) = doc["start_date"].as_str() {
                start_date = Some(
                    NaiveDate::parse_from_str(d, "%Y/%m/%d").expect("parse error of start_date"),
                );
            }
            let mut due: Option<NaiveDate> = None;
            if let Some(d) = doc["due"].as_str() {
                due = Some(NaiveDate::parse_from_str(d, "%Y/%m/%d").expect("parse error of due"));
            }
            let progress = doc["progress"].as_i64().expect("no progress") as u16;
            comps.push(RelativeIssueComponent {
                id,
                complete: id_inactive,
                title,
                status,
                person_in_charge,
                start_date,
                due,
                progress,
            })
        }
        comps
    }
}

impl Component for RelativeIssueComponent {
    fn line_count(&self, _: u16) -> u16 {
        1
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
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
