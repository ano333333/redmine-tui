use std::cmp::min;

use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueChildrenListComponent {
    id: u16,
}

impl IssueChildrenListComponent {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
    pub fn render(&self, store: &Store, max_width: u16, max_height: u16) -> Option<Buffer> {
        if let Some(issue) = store.get_issue(self.id) {
            let child_all_num = issue.child_ids.len() as u16;
            let child_complete_num = issue
                .child_ids
                .iter()
                .map(|id| store.get_issue(*id))
                .filter(|issue| issue.is_some_and(|issue| issue.is_completed()))
                .count() as u16;
            let child_imcomplete_num = child_all_num - child_complete_num;

            let mut area = Rect::new(
                0,
                0,
                max_width,
                min(2 + (issue.child_ids.len() as u16) + 1, max_height),
            );
            let mut buffer = Buffer::empty(area);

            let header_text =
                create_header_widget(child_all_num, child_complete_num, child_imcomplete_num);
            header_text.render(area, &mut buffer);
            area.y += 2;
            area.height = area.height.saturating_sub(2);

            for child in &issue.child_ids {
                if let Some(child) = store.get_issue(*child) {
                    render_children_issue(child, area, &mut buffer);
                    area.y += 1;
                    area.height = area.height.saturating_sub(1);
                }
            }

            Some(buffer)
        } else {
            None
        }
    }
    pub fn line_count(&self, store: &Store) -> u16 {
        if let Some(issue) = store.get_issue(self.id) {
            2 + (issue.child_ids.len() as u16) + 1
        } else {
            0
        }
    }
}

fn create_header_widget(
    child_all_num: u16,
    child_complete_num: u16,
    child_imcomplete_num: u16,
) -> Text<'static> {
    let child_header_title = Line::from(vec![
        Span::from("子チケット"),
        Span::from(" "),
        Span::from(format!(
            "{} ({}件未完了 - {}件完了)",
            child_all_num, child_imcomplete_num, child_complete_num
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
    create_id_widget(issue.id, &issue.status).render(cols[0], buffer);
    create_title_widget(&issue.title).render(cols[2], buffer);
    create_status_widget(&issue.status).render(cols[4], buffer);
    create_person_in_charge_widgte(&issue.person_in_charge).render(cols[6], buffer);
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

fn create_person_in_charge_widgte(person_in_charge: &Option<String>) -> Paragraph<'static> {
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
