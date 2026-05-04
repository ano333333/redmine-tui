use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::Store;
use crate::entities::Issue;

pub struct IssuePropertyComponent {
    id: u16,
}

impl IssuePropertyComponent {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
    pub fn render(&self, store: &Store, frame: &mut Frame, area: &mut Rect) {
        if let Some(issue) = store.get_issue(self.id) {
            let paragraphs = create_widgets(issue);
            for p in paragraphs.iter() {
                frame.render_widget(p, *area);
                let l = p.line_count(area.width) as u16;
                area.y += l;
                area.height = area.height.saturating_sub(l);
            }
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
