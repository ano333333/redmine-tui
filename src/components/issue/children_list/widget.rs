use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::entities::Issue;

pub struct ChildrenListWidget<'a> {
    pub child_all_num: u16,
    pub child_complete_num: u16,
    pub child_incomplete_num: u16,
    pub children: Vec<&'a Issue>,
}

impl<'a> Widget for ChildrenListWidget<'a> {
    fn render(self, mut area: Rect, buf: &mut Buffer) {
        let header_text = create_header_text(
            self.child_all_num,
            self.child_complete_num,
            self.child_incomplete_num,
        );
        header_text.render(area, buf);
        area.y += 2;
        area.height = area.height.saturating_sub(2);

        for child in &self.children {
            render_children_issue(child, area, buf);
            area.y += 1;
            area.height = area.height.saturating_sub(1);
        }
    }
}

impl<'a> ChildrenListWidget<'a> {
    pub fn line_count(&self) -> u16 {
        2 + self.child_all_num + 1
    }
}

fn create_header_text(
    child_all_num: u16,
    child_complete_num: u16,
    child_incomplete_num: u16,
) -> Text<'static> {
    let child_header_title = Line::from(vec![
        Span::from("子チケット"),
        Span::from(" "),
        Span::from(format!(
            "{} ({}件未完了 - {}件完了)",
            child_all_num, child_incomplete_num, child_complete_num
        )),
    ]);
    Text::from(vec![child_header_title, Line::from("")])
}

fn render_children_issue(issue: &Issue, area: Rect, buffer: &mut Buffer) {
    let row = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(1)])
        .split(area)[0];
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(7),  // ID
            Constraint::Length(1),
            Constraint::Fill(1),    // title
            Constraint::Length(1),
            Constraint::Length(7),  // status
            Constraint::Length(1),
            Constraint::Length(12), // person_in_charge
            Constraint::Length(1),
            Constraint::Length(10), // created_at
            Constraint::Length(1),
            Constraint::Length(10), // due
            Constraint::Length(1),
            Constraint::Length(4),  // progress
        ])
        .split(row);
    create_id_widget(issue.id, &issue.status).render(cols[0], buffer);
    create_title_widget(&issue.title).render(cols[2], buffer);
    create_status_widget(&issue.status).render(cols[4], buffer);
    create_person_in_charge_widget(&issue.person_in_charge).render(cols[6], buffer);
    create_start_date_widget(&issue.start_date).render(cols[8], buffer);
    create_due_widget(&issue.due).render(cols[10], buffer);
    create_progress_widget(issue.progress).render(cols[12], buffer);
}

fn create_id_widget(id: u16, status: &String) -> Paragraph<'static> {
    let id_style = if status == "完了" {
        Style::default().add_modifier(Modifier::CROSSED_OUT).gray()
    } else {
        Style::default().blue()
    };
    Paragraph::new(Text::from(format!("#{}", id))).style(id_style)
}

fn create_title_widget(title: &String) -> Paragraph<'static> {
    Paragraph::new(Text::from(title.clone())).wrap(Wrap { trim: true })
}

fn create_status_widget(status: &String) -> Paragraph<'static> {
    Paragraph::new(Text::from(status.clone())).wrap(Wrap { trim: true })
}

fn create_person_in_charge_widget(person_in_charge: &Option<String>) -> Paragraph<'static> {
    match &person_in_charge {
        Some(p) => Paragraph::new(Text::from(p.clone())).style(Style::default().fg(Color::Blue)),
        None => Paragraph::new(Text::from("(なし)")).style(Style::default().fg(Color::Gray)),
    }
}

fn create_start_date_widget(start_date: &Option<DateTime<Local>>) -> Paragraph<'static> {
    match &start_date {
        Some(date) => Paragraph::new(Text::from(date.format("%Y/%m/%d").to_string())),
        None => Paragraph::new(Text::from("----/--/--".to_string()))
            .style(Style::default().fg(Color::Gray)),
    }
}

fn create_due_widget(due: &Option<DateTime<Local>>) -> Paragraph<'static> {
    match &due {
        Some(due) => Paragraph::new(Text::from(due.format("%Y/%m/%d").to_string())),
        None => Paragraph::new(Text::from("----/--/--".to_string()))
            .style(Style::default().fg(Color::Gray)),
    }
}

fn create_progress_widget(progress: u16) -> Paragraph<'static> {
    Paragraph::new(Text::from(format!("{:>3}%", progress.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{render_snapshot, sample_issue};

    #[test]
    fn snapshot_children_mixed_option_and_status_display() {
        let done = sample_issue(
            7,
            "Done child",
            "完了",
            Some("alice"),
            Some("2026-01-10T00:00:00+09:00"),
            Some("2026-01-15T00:00:00+09:00"),
            100,
        );
        let open = sample_issue(
            8,
            "Open child without assignee and dates",
            "進行中",
            None,
            None,
            None,
            35,
        );
        render_snapshot(
            "children_mixed_option_and_status_display",
            64,
            5,
            ChildrenListWidget {
                child_all_num: 2,
                child_complete_num: 1,
                child_incomplete_num: 1,
                children: vec![&done, &open],
            },
        );
    }

    #[test]
    fn line_count_children_current_values() {
        let child_a = sample_issue(7, "Done child", "完了", Some("alice"), None, None, 100);
        let child_b = sample_issue(8, "Open child", "進行中", None, None, None, 35);
        let widget = ChildrenListWidget {
            child_all_num: 2,
            child_complete_num: 1,
            child_incomplete_num: 1,
            children: vec![&child_a, &child_b],
        };
        assert_eq!(widget.line_count(), 5);
    }
}
