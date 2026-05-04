use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::Store;
use crate::entities::Issue;

pub struct IssueChildrenListComponent {
    id: u16,
}

impl IssueChildrenListComponent {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
    pub fn render(&self, store: &Store, frame: &mut Frame, area: &mut Rect) {
        let issue = store.get_issue(self.id);
        if let Some(issue) = issue {
            let child_all_num = issue.relative_ids.len() as u16;
            let child_complete_num = issue
                .relative_ids
                .iter()
                .map(|id| store.get_issue(*id))
                .filter(|issue| {
                    if let Some(issue) = issue {
                        issue.status == "完了"
                    } else {
                        false
                    }
                })
                .count() as u16;
            let child_imcomplete_num = child_all_num - child_complete_num;
            let header_widgets =
                create_header_widget(child_all_num, child_complete_num, child_imcomplete_num);
            frame.render_widget(header_widgets, *area);
            area.y += 2;
            area.height = area.height.saturating_sub(2);

            for child in &issue.relative_ids {
                if let Some(child) = store.get_issue(*child) {
                    render_children_issue(child, frame, area);
                    area.y += 1;
                    area.height = area.height.saturating_sub(1);
                }
            }

            area.y += 1;
            area.height = area.height.saturating_sub(1);
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

fn render_children_issue(issue: &Issue, frame: &mut Frame, area: &mut Rect) {
    let row = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(1)])
        .split(*area)[0];
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

    frame.render_widget(create_id_widget(issue.id, &issue.status), cols[0]);
    frame.render_widget(create_title_widget(&issue.title), cols[2]);
    frame.render_widget(create_status_widget(&issue.status), cols[4]);
    frame.render_widget(
        create_person_in_charge_widgte(&issue.person_in_charge),
        cols[6],
    );
    frame.render_widget(create_start_date_widget(&issue.start_date), cols[8]);
    frame.render_widget(create_due_widget(&issue.due), cols[10]);
    frame.render_widget(create_progress_widget(issue.progress), cols[12]);
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
