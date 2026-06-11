use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::app::IssueStatusState;
use crate::entities::{Issue, IssueStatus};

// TODO: Extract this focus background color into one shared constant for all widgets.
const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct ChildIssueRow<'a> {
    pub issue: &'a Issue,
    pub issue_status: &'a (IssueStatus, IssueStatusState),
}

pub struct ChildrenListWidget<'a> {
    children_num: u16,
    closed_children_num: u16,
    open_children_num: u16,
    children: Vec<ChildIssueRow<'a>>,
    focused_index: Option<usize>,
}

impl<'a> Widget for ChildrenListWidget<'a> {
    fn render(self, mut area: Rect, buf: &mut Buffer) {
        let header_text = create_header_text(
            self.children_num,
            self.closed_children_num,
            self.open_children_num,
        );
        header_text.render(area, buf);
        area.y += 2;
        area.height = area.height.saturating_sub(2);

        let focused_index = self.focused_index;
        for (index, child) in self.children.iter().enumerate() {
            render_children_issue(child, area, buf, focused_index == Some(index));
            area.y += 1;
            area.height = area.height.saturating_sub(1);
        }
    }
}

impl<'a> ChildrenListWidget<'a> {
    pub fn new(
        children_num: u16,
        closed_children_num: u16,
        open_children_num: u16,
        children: Vec<ChildIssueRow<'a>>,
        focused_index: Option<usize>,
    ) -> Self {
        Self {
            children_num,
            closed_children_num,
            open_children_num,
            children,
            focused_index,
        }
    }

    pub fn line_count(&self) -> u16 {
        2 + self.children_num + 1
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

fn render_children_issue(child: &ChildIssueRow, area: Rect, buffer: &mut Buffer, focused: bool) {
    let issue = child.issue;
    let status_name = child.issue_status.0.name.as_str();
    let is_closed = child.issue_status.0.is_closed;
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
    create_id_widget(issue.id, is_closed).render(cols[0], buffer);
    create_title_widget(&issue.title).render(cols[2], buffer);
    create_status_widget(status_name).render(cols[4], buffer);
    create_person_in_charge_widget(&issue.person_in_charge).render(cols[6], buffer);
    create_start_date_widget(&issue.start_date).render(cols[8], buffer);
    create_due_widget(&issue.due).render(cols[10], buffer);
    create_progress_widget(issue.progress).render(cols[12], buffer);

    if focused {
        for x in 0..row.width {
            if let Some(cell) = buffer.cell_mut((row.x + x, row.y)) {
                cell.set_bg(FOCUS_BG);
            }
        }
    }
}

fn create_id_widget(id: u16, is_closed: bool) -> Paragraph<'static> {
    let id_style = if is_closed {
        Style::default().add_modifier(Modifier::CROSSED_OUT).gray()
    } else {
        Style::default().blue()
    };
    Paragraph::new(Text::from(format!("#{}", id))).style(id_style)
}

fn create_title_widget(title: &String) -> Paragraph<'static> {
    Paragraph::new(Text::from(title.clone())).wrap(Wrap { trim: true })
}

fn create_status_widget(status: &str) -> Paragraph<'static> {
    Paragraph::new(Text::from(status.to_string())).wrap(Wrap { trim: true })
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
            3,
            Some("alice"),
            Some("2026-01-10T00:00:00+09:00"),
            Some("2026-01-15T00:00:00+09:00"),
            100,
        );
        let open = sample_issue(
            8,
            "Open child without assignee and dates",
            5,
            None,
            None,
            None,
            35,
        );
        render_snapshot(
            "children_mixed_option_and_status_display",
            64,
            5,
            ChildrenListWidget::new(
                2,
                1,
                1,
                vec![
                    ChildIssueRow {
                        issue: &done,
                        issue_status: &(
                            IssueStatus {
                                id: 3,
                                name: "完了(closed)".to_string(),
                                is_closed: true,
                            },
                            IssueStatusState::Existing,
                        ),
                    },
                    ChildIssueRow {
                        issue: &open,
                        issue_status: &(
                            IssueStatus {
                                id: 5,
                                name: "進行中(accepted)".to_string(),
                                is_closed: false,
                            },
                            IssueStatusState::Existing,
                        ),
                    },
                ],
                Some(1),
            ),
        );
    }

    #[test]
    fn line_count_children_current_values() {
        let child_a = sample_issue(7, "Done child", 3, Some("alice"), None, None, 100);
        let child_b = sample_issue(8, "Open child", 5, None, None, None, 35);
        let child_a_status = (
            IssueStatus {
                id: 3,
                name: "完了(closed)".to_string(),
                is_closed: true,
            },
            IssueStatusState::Existing,
        );
        let child_b_status = (
            IssueStatus {
                id: 5,
                name: "進行中(accepted)".to_string(),
                is_closed: false,
            },
            IssueStatusState::Existing,
        );
        let widget = ChildrenListWidget::new(
            2,
            1,
            1,
            vec![
                ChildIssueRow {
                    issue: &child_a,
                    issue_status: &child_a_status,
                },
                ChildIssueRow {
                    issue: &child_b,
                    issue_status: &child_b_status,
                },
            ],
            None,
        );
        assert_eq!(widget.line_count(), 5);
    }
}
