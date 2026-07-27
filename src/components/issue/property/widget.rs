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
    author: &'a str,
    created_on: DateTime<Local>,
    updated_on: DateTime<Local>,
    status: &'a str,
    tracker: &'a str,
    priority: &'a str,
    project: &'a str,
    person_in_charge: Option<&'a str>,
    target_version: Option<&'a str>,
    start_date: Option<DateTime<Local>>,
    due: Option<DateTime<Local>>,
    progress: u16,
    planned_hours: Option<u16>,
    total_spent_hours: Option<f64>,
    category: &'a str,
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
        author: &'a str,
        created_on: DateTime<Local>,
        updated_on: DateTime<Local>,
        status: &'a str,
        tracker: &'a str,
        priority: &'a str,
        project: &'a str,
        person_in_charge: Option<&'a str>,
        target_version: Option<&'a str>,
        start_date: Option<DateTime<Local>>,
        due: Option<DateTime<Local>>,
        progress: u16,
        planned_hours: Option<u16>,
        total_spent_hours: Option<f64>,
        category: &'a str,
        focused_y: Option<u16>,
    ) -> Self {
        Self {
            id,
            author,
            created_on,
            updated_on,
            status,
            tracker,
            priority,
            project,
            person_in_charge,
            target_version,
            start_date,
            due,
            progress,
            planned_hours,
            total_spent_hours,
            category,
            focused_y,
        }
    }

    pub fn line_count(&self, width: u16) -> usize {
        let paragraph = self.create_paragraph();
        paragraph.line_count(width)
    }

    fn create_paragraph(&self) -> Paragraph<'_> {
        let person = match self.person_in_charge {
            Some(s) => s.to_string(),
            None => "-".to_string(),
        };
        let target_version = match self.target_version {
            Some(s) => s.to_string(),
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
        let total_spent_hours = match self.total_spent_hours {
            Some(hours) if hours.fract() == 0.0 => format!("{hours:.0}"),
            Some(hours) => hours.to_string(),
            None => "".to_string(),
        };
        Paragraph::new(vec![
            Line::from(format!("作成者              {}", self.author)),
            Line::from(format!(
                "作成日              {}",
                self.created_on.format("%Y/%m/%d")
            )),
            Line::from(format!(
                "更新日              {}",
                self.updated_on.format("%Y/%m/%d")
            )),
            Line::from(vec![
                Span::from("ステータス          ").style(Style::default().blue()),
                Span::from(self.status.to_string()),
            ]),
            Line::from(format!("トラッカー          {}", self.tracker)),
            Line::from(format!("優先度              {}", self.priority)),
            Line::from(format!("プロジェクト        {}", self.project)),
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
            Line::from(format!("実績工数            {}", total_spent_hours)),
            Line::from(format!("カテゴリー          {}", self.category)),
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
        let tracker = "Bug".to_string();
        let priority = "critical".to_string();
        let project = "Sample Project".to_string();
        let person_in_charge = Some("alice".to_string());
        let target_version = Some("2026 Spring".to_string());
        let category = "Admin UI".to_string();
        render_snapshot(
            "property_full_values_wide",
            40,
            15,
            PropertyWidget::new(
                1,
                "author",
                local_datetime("2026-01-10T00:00:00+09:00"),
                local_datetime("2026-01-15T00:00:00+09:00"),
                status.as_str(),
                tracker.as_str(),
                priority.as_str(),
                project.as_str(),
                person_in_charge.as_deref(),
                target_version.as_deref(),
                Some(local_datetime("2026-01-10T00:00:00+09:00")),
                Some(local_datetime("2026-01-20T00:00:00+09:00")),
                65,
                Some(13),
                Some(8.5),
                category.as_str(),
                Some(3),
            ),
        );
    }

    #[test]
    fn snapshot_property_all_optional_none() {
        let status = "Waiting for external review".to_string();
        let tracker = "Support".to_string();
        let priority = "major".to_string();
        let project = "Sample Project".to_string();
        let person_in_charge = None;
        let target_version: Option<String> = None;
        let category = "Operations Integration".to_string();
        render_snapshot(
            "property_all_optional_none",
            22,
            17,
            PropertyWidget::new(
                1,
                "author",
                local_datetime("2026-01-10T00:00:00+09:00"),
                local_datetime("2026-01-15T00:00:00+09:00"),
                status.as_str(),
                tracker.as_str(),
                priority.as_str(),
                project.as_str(),
                person_in_charge,
                target_version.as_deref(),
                None,
                None,
                0,
                None,
                None,
                category.as_str(),
                None,
            ),
        );
    }

    #[test]
    fn line_count_property_current_values() {
        let status = "Waiting for external review".to_string();
        let tracker = "Support".to_string();
        let priority = "minor".to_string();
        let project = "Sample Project".to_string();
        let person_in_charge = None;
        let target_version: Option<String> = None;
        let category = "Operations Integration".to_string();
        let widget = PropertyWidget::new(
            1,
            "author",
            local_datetime("2026-01-10T00:00:00+09:00"),
            local_datetime("2026-01-15T00:00:00+09:00"),
            status.as_str(),
            tracker.as_str(),
            priority.as_str(),
            project.as_str(),
            person_in_charge,
            target_version.as_deref(),
            None,
            None,
            0,
            None,
            None,
            category.as_str(),
            None,
        );
        assert_eq!(widget.line_count(40), 15);
        assert_eq!(widget.line_count(22), 15);
    }

    #[test]
    fn line_count_property_is_stable_without_wrap() {
        let status = "Waiting for external review".to_string();
        let tracker = "Support".to_string();
        let priority = "blocker".to_string();
        let project = "Sample Project".to_string();
        let person_in_charge = None;
        let target_version: Option<String> = None;
        let category = "Operations Integration".to_string();
        let widget = PropertyWidget::new(
            1,
            "author",
            local_datetime("2026-01-10T00:00:00+09:00"),
            local_datetime("2026-01-15T00:00:00+09:00"),
            status.as_str(),
            tracker.as_str(),
            priority.as_str(),
            project.as_str(),
            person_in_charge,
            target_version.as_deref(),
            None,
            None,
            0,
            None,
            None,
            category.as_str(),
            None,
        );
        assert_eq!(widget.line_count(40), widget.line_count(22));
    }
}
