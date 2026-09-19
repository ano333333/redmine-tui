use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::entities::{IssueAggregate, IssueStatus, IssueStatusExt};
use crate::vos::IssueId;
use crate::widgets::gutter::{Gutter, indented_area};
use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED, SECTION_BAR};

/// 見出しとその下の空行。縦線はこの下の一覧部分にだけ引く。
pub const HEADER_LINES: u16 = 2;
/// 縦線を引かない末尾の余白行。次のブロックとの区切りになる。
const GUTTER_TRAILING_LINES: u16 = 1;

pub struct ChildIssueRow<'a> {
    pub issue: &'a IssueAggregate,
    pub issue_status: Option<&'a IssueStatus>,
    pub assigned_to_name: Option<&'a str>,
}

pub struct ChildrenListWidget<'a> {
    children_num: u16,
    closed_children_num: u16,
    open_children_num: u16,
    children: Vec<ChildIssueRow<'a>>,
    focused_index: Option<usize>,
}

impl<'a> Widget for ChildrenListWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let header_text = create_header_text(
            self.children_num,
            self.closed_children_num,
            self.open_children_num,
        );
        header_text.render(area, buf);

        // 縦線と字下げは見出しの下、一覧の部分だけに掛ける
        let list_area = Rect::new(
            area.x,
            area.y + HEADER_LINES,
            area.width,
            area.height.saturating_sub(HEADER_LINES),
        );
        let gutter_area = Rect::new(
            list_area.x,
            list_area.y,
            list_area.width,
            list_area.height.min(
                self.line_count()
                    .saturating_sub(HEADER_LINES + GUTTER_TRAILING_LINES),
            ),
        );
        let gutter = Gutter::line();
        gutter.render(gutter_area, buf);

        let mut row_area = indented_area(list_area);
        let focused_index = self.focused_index;
        for (index, child) in self.children.iter().enumerate() {
            render_children_issue(child, row_area, buf, focused_index == Some(index));
            row_area.y += 1;
            row_area.height = row_area.height.saturating_sub(1);
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
        // 見出し2行 + 子チケット + 縦線を引かない末尾の余白1行
        HEADER_LINES + self.children_num + 1
    }
}

fn create_header_text(
    child_all_num: u16,
    child_complete_num: u16,
    child_incomplete_num: u16,
) -> Text<'static> {
    let child_header_title = Line::from(vec![
        Span::from(SECTION_BAR).fg(ACCENT),
        Span::from(" 子チケット ").bold(),
        Span::from(format!("{child_all_num}")).fg(ACCENT).bold(),
        Span::from(format!(
            "  未完了 {child_incomplete_num} / 完了 {child_complete_num}"
        ))
        .fg(MUTED),
    ]);
    Text::from(vec![child_header_title, Line::from("")])
}

fn render_children_issue(child: &ChildIssueRow, area: Rect, buffer: &mut Buffer, focused: bool) {
    let issue = child.issue;
    let status_name = child
        .issue_status
        .map_or("(不明)", |issue_status| issue_status.name.as_str());
    let is_closed = child.issue_status.is_closed_status();
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
    create_id_widget(issue.issue.id, is_closed).render(cols[0], buffer);
    create_title_widget(&issue.issue.subject).render(cols[2], buffer);
    create_status_widget(status_name).render(cols[4], buffer);
    create_person_in_charge_widget(child.assigned_to_name).render(cols[6], buffer);
    create_start_date_widget(&issue.start_date).render(cols[8], buffer);
    create_due_widget(&issue.due_date).render(cols[10], buffer);
    create_progress_widget(issue.done_ratio).render(cols[12], buffer);

    if focused {
        for x in 0..row.width {
            if let Some(cell) = buffer.cell_mut((row.x + x, row.y)) {
                cell.set_bg(FOCUS_BG);
            }
        }
    }
}

fn create_id_widget(id: IssueId, is_closed: bool) -> Paragraph<'static> {
    let id_style = if is_closed {
        Style::default().add_modifier(Modifier::CROSSED_OUT).gray()
    } else {
        Style::default().blue()
    };
    Paragraph::new(Text::from(format!("#{}", id))).style(id_style)
}

fn create_title_widget(title: &str) -> Paragraph<'static> {
    Paragraph::new(Text::from(title.to_string())).wrap(Wrap { trim: true })
}

fn create_status_widget(status: &str) -> Paragraph<'static> {
    Paragraph::new(Text::from(status.to_string())).wrap(Wrap { trim: true })
}

fn create_person_in_charge_widget(person_in_charge: Option<&str>) -> Paragraph<'static> {
    match person_in_charge {
        Some(p) => {
            Paragraph::new(Text::from(p.to_string())).style(Style::default().fg(Color::Blue))
        }
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
    use crate::test_support::{render_snapshot, sample_issue_aggregate};

    #[test]
    fn snapshot_children_mixed_option_and_status_display() {
        let done = sample_issue_aggregate(
            7,
            "Done child",
            3.into(),
            Some(1),
            Some("2026-01-10T00:00:00+09:00"),
            Some("2026-01-15T00:00:00+09:00"),
            100,
        );
        let open = sample_issue_aggregate(
            8,
            "Open child without assignee and dates",
            5.into(),
            None,
            None,
            None,
            35,
        );
        let unknown = sample_issue_aggregate(
            9,
            "Child with unknown status",
            9.into(),
            None,
            None,
            None,
            0,
        );
        let done_status = IssueStatus {
            id: 3.into(),
            name: "完了(closed)".to_string(),
            is_closed: true,
        };
        let open_status = IssueStatus {
            id: 5.into(),
            name: "進行中(accepted)".to_string(),
            is_closed: false,
        };
        render_snapshot(
            "children_mixed_option_and_status_display",
            64,
            6,
            ChildrenListWidget::new(
                3,
                1,
                2,
                vec![
                    ChildIssueRow {
                        issue: &done,
                        issue_status: Some(&done_status),
                        assigned_to_name: Some("alice"),
                    },
                    ChildIssueRow {
                        issue: &open,
                        issue_status: Some(&open_status),
                        assigned_to_name: None,
                    },
                    ChildIssueRow {
                        issue: &unknown,
                        issue_status: None,
                        assigned_to_name: None,
                    },
                ],
                Some(1),
            ),
        );
    }

    #[test]
    fn line_count_children_current_values() {
        let child_a = sample_issue_aggregate(7, "Done child", 3.into(), Some(1), None, None, 100);
        let child_b = sample_issue_aggregate(8, "Open child", 5.into(), None, None, None, 35);
        let child_a_status = IssueStatus {
            id: 3.into(),
            name: "完了(closed)".to_string(),
            is_closed: true,
        };
        let child_b_status = IssueStatus {
            id: 5.into(),
            name: "進行中(accepted)".to_string(),
            is_closed: false,
        };
        let child_c = sample_issue_aggregate(9, "Unknown child", 9.into(), None, None, None, 0);
        let widget = ChildrenListWidget::new(
            3,
            1,
            2,
            vec![
                ChildIssueRow {
                    issue: &child_a,
                    issue_status: Some(&child_a_status),
                    assigned_to_name: Some("alice"),
                },
                ChildIssueRow {
                    issue: &child_b,
                    issue_status: Some(&child_b_status),
                    assigned_to_name: None,
                },
                ChildIssueRow {
                    issue: &child_c,
                    issue_status: None,
                    assigned_to_name: None,
                },
            ],
            None,
        );
        assert_eq!(widget.line_count(), 6);
    }
}
