use std::cmp::min;

use chrono::{DateTime, Local};
use crossterm::event::{Event, KeyCode};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssuePropertyComponent {
    id: u16,
    focused_y: Option<u16>,
}

pub enum FocusTransitionEvent {
    Unfocused,
    CursorEnteredFromAbove,
    CursorEnteredFromBelow,
}

pub enum EventProcessResult {
    CursorLeavedFromAbove,
    CursorLeavedFromBelow,
}

impl IssuePropertyComponent {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            focused_y: None,
        }
    }
    pub fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        if let Some(issue) = store.get_issue(self.id) {
            let mut area = Rect::new(0, 0, max_width, min(11, max_height));
            let mut buffer = Buffer::empty(area);
            let paragraphs = create_widgets(issue);
            for p in paragraphs.iter() {
                p.render(area, &mut buffer);
                let l = p.line_count(area.width) as u16;
                area.y += l;
                area.height = area.height.saturating_sub(l);
            }
            return Some(buffer);
        }
        None
    }
    pub fn line_count(&self, store: &Store, width: u16) -> u16 {
        if let Some(issue) = store.get_issue(self.id) {
            let paragraphs = create_widgets(issue);
            paragraphs
                .iter()
                .fold(0, |acc, iter| acc + iter.line_count(width) as u16)
        } else {
            0
        }
    }
    pub fn focus_event(&mut self, event: FocusTransitionEvent) {
        match event {
            FocusTransitionEvent::Unfocused => {
                self.focused_y = None;
            }
            FocusTransitionEvent::CursorEnteredFromAbove => {
                self.focused_y = Some(0);
            }
            FocusTransitionEvent::CursorEnteredFromBelow => {
                self.focused_y = Some(10);
            }
        }
    }
    pub fn process_event(&mut self, event: &Event) -> Option<EventProcessResult> {
        let focused_y = self.focused_y.unwrap();
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('j') => {
                    if focused_y + 1 == 11 {
                        return Some(EventProcessResult::CursorLeavedFromBelow);
                    }
                    self.focused_y = Some(focused_y + 1);
                }
                KeyCode::Char('k') => {
                    if focused_y == 0 {
                        return Some(EventProcessResult::CursorLeavedFromAbove);
                    }
                    self.focused_y = Some(focused_y - 1);
                }
                _ => {}
            }
        }
        None
    }
    pub fn get_cursor_position(&self) -> Position {
        Position {
            x: 20,
            y: self.focused_y.unwrap(),
        }
    }
}

fn create_widgets(issue: &Issue) -> Vec<Paragraph<'static>> {
    let person = match &issue.person_in_charge {
        Some(s) => s.clone(),
        None => "-".to_string(),
    };
    let target_version = match &issue.target_version {
        Some(s) => s.clone(),
        None => "-".to_string(),
    };
    fn datetime_opt_to_str(date_opt: &Option<DateTime<Local>>) -> String {
        match date_opt {
            Some(d) => d.format("%Y/%m/%d").to_string(),
            None => "-".to_string(),
        }
    }
    let planned_hours = match issue.planned_hours {
        Some(p) => p.to_string(),
        None => "".to_string(),
    };
    let resolve_way = match &issue.resolve_way {
        Some(r) => r.clone(),
        None => "-".to_string(),
    };
    vec![Paragraph::new(vec![
        Line::from(vec![
            Span::from("ステータス          ").style(Style::default().blue()),
            Span::from(issue.status.clone()),
        ]),
        Line::from(format!("優先度              {}", issue.priority.clone())),
        Line::from(format!("担当者              {}", person)).style(Style::default().blue()),
        Line::from(format!("対象バージョン      {}", target_version)),
        Line::from(format!(
            "開始日              {}",
            datetime_opt_to_str(&issue.start_date)
        )),
        Line::from(format!(
            "期日                {}",
            datetime_opt_to_str(&issue.due)
        )),
        Line::from(vec![
            Span::from("進捗率              ").style(Style::default().blue()),
            Span::from(issue.progress.to_string()),
        ]),
        Line::from(format!("予定工数            {}", planned_hours)),
        Line::from(format!("解決方法            {}", resolve_way)),
        Line::from(format!("コンポーネント      {}", &issue.component)),
        Line::from(format!("Tags                {}", issue.tags.concat())),
    ])]
}
