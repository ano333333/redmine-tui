use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct PropertyWidget<'a> {
    id: u16,
    status: &'a str,
    priority: &'a String,
    person_in_charge: &'a Option<String>,
    target_version: &'a Option<String>,
    start_date: Option<DateTime<Local>>,
    due: Option<DateTime<Local>>,
    progress: u16,
    planned_hours: Option<u16>,
    resolve_way: &'a Option<String>,
    component: &'a String,
    tags: &'a Vec<String>,
    focused_y: Option<u16>,
}

impl<'a> Widget for PropertyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let focused_y = self.focused_y;
        let paragraph = self.create_paragraph();
        paragraph.render(area, buf);
        if let Some(row) = focused_y {
            apply_background_to_row(buf, area, row);
        }
    }
}

impl<'a> PropertyWidget<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u16,
        status: &'a str,
        priority: &'a String,
        person_in_charge: &'a Option<String>,
        target_version: &'a Option<String>,
        start_date: Option<DateTime<Local>>,
        due: Option<DateTime<Local>>,
        progress: u16,
        planned_hours: Option<u16>,
        resolve_way: &'a Option<String>,
        component: &'a String,
        tags: &'a Vec<String>,
        focused_y: Option<u16>,
    ) -> Self {
        Self {
            id,
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
            focused_y,
        }
    }

    pub fn line_count(&self, width: u16) -> usize {
        let paragraph = self.create_paragraph();
        paragraph.line_count(width)
    }

    fn create_paragraph(&self) -> Paragraph<'_> {
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
                Span::from(self.status.to_string()),
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

fn apply_background_to_row(buf: &mut Buffer, area: Rect, row: u16) {
    if row >= area.height {
        return;
    }

    for x in 0..area.width {
        if let Some(cell) = buf.cell_mut((area.x + x, area.y + row)) {
            cell.set_bg(FOCUS_BG);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};

    #[test]
    fn snapshot_property_full_values_wide() {
        let status = "In Progress".to_string();
        let priority = "Urgent".to_string();
        let person_in_charge = Some("alice".to_string());
        let target_version = Some("2026 Spring".to_string());
        let resolve_way = Some("Patch".to_string());
        let component = "Admin UI".to_string();
        let tags = vec!["frontend".to_string(), "triage".to_string()];
        render_snapshot(
            "property_full_values_wide",
            40,
            11,
            PropertyWidget::new(
                1,
                status.as_str(),
                &priority,
                &person_in_charge,
                &target_version,
                Some(local_datetime("2026-01-10T00:00:00+09:00")),
                Some(local_datetime("2026-01-20T00:00:00+09:00")),
                65,
                Some(13),
                &resolve_way,
                &component,
                &tags,
                Some(2),
            ),
        );
    }

    #[test]
    fn snapshot_property_all_optional_none() {
        let status = "Waiting for external review".to_string();
        let priority = "Very high".to_string();
        let person_in_charge = None;
        let target_version = None;
        let resolve_way = None;
        let component = "Operations Integration".to_string();
        let tags = vec!["frontend".to_string(), "needs-review".to_string()];
        render_snapshot(
            "property_all_optional_none",
            22,
            16,
            PropertyWidget::new(
                1,
                status.as_str(),
                &priority,
                &person_in_charge,
                &target_version,
                None,
                None,
                0,
                None,
                &resolve_way,
                &component,
                &tags,
                None,
            ),
        );
    }

    #[test]
    fn line_count_property_current_values() {
        let status = "Waiting for external review".to_string();
        let priority = "Very high".to_string();
        let person_in_charge = None;
        let target_version = None;
        let resolve_way = None;
        let component = "Operations Integration".to_string();
        let tags = vec!["frontend".to_string(), "needs-review".to_string()];
        let widget = PropertyWidget::new(
            1,
            status.as_str(),
            &priority,
            &person_in_charge,
            &target_version,
            None,
            None,
            0,
            None,
            &resolve_way,
            &component,
            &tags,
            None,
        );
        assert_eq!(widget.line_count(40), 11);
        assert_eq!(widget.line_count(22), 11);
    }

    #[test]
    fn line_count_property_is_stable_without_wrap() {
        let status = "Waiting for external review".to_string();
        let priority = "Very high".to_string();
        let person_in_charge = None;
        let target_version = None;
        let resolve_way = None;
        let component = "Operations Integration".to_string();
        let tags = vec!["frontend".to_string(), "needs-review".to_string()];
        let widget = PropertyWidget::new(
            1,
            status.as_str(),
            &priority,
            &person_in_charge,
            &target_version,
            None,
            None,
            0,
            None,
            &resolve_way,
            &component,
            &tags,
            None,
        );
        assert_eq!(widget.line_count(40), widget.line_count(22));
    }
}
