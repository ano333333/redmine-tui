use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct PropertyWidget<'a> {
    pub id: u16,
    pub status: &'a String,
    pub priority: &'a String,
    pub person_in_charge: &'a Option<String>,
    pub target_version: &'a Option<String>,
    pub start_date: Option<DateTime<Local>>,
    pub due: Option<DateTime<Local>>,
    pub progress: u16,
    pub planned_hours: Option<u16>,
    pub resolve_way: &'a Option<String>,
    pub component: &'a String,
    pub tags: &'a Vec<String>,
}

impl<'a> Widget for PropertyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragraph = self.create_paragraph();
        paragraph.render(area, buf);
    }
}

impl<'a> PropertyWidget<'a> {
    pub fn line_count(&self, width: u16) -> usize {
        let paragraph = self.create_paragraph();
        paragraph.line_count(width)
    }

    fn create_paragraph(&self) -> Paragraph {
        let person = match self.person_in_charge {
            Some(s) => s.clone(),
            None => "-".to_string(),
        };
        let target_version = match self.target_version {
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
        let resolve_way = match self.resolve_way {
            Some(r) => r.clone(),
            None => "-".to_string(),
        };
        Paragraph::new(vec![
            Line::from(vec![
                Span::from("ステータス          ").style(Style::default().blue()),
                Span::from(self.status.clone()),
            ]),
            Line::from(format!("優先度              {}", self.priority.clone())),
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
            Line::from(format!("コンポーネント      {}", self.component)),
            Line::from(format!("Tags                {}", self.tags.concat())),
        ])
    }
}
