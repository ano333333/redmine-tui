use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const SELECTED_BG: Color = Color::Rgb(0x22, 0x22, 0x22);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueSelectPopupProject {
    pub id: u16,
    pub name: String,
}

impl IssueSelectPopupProject {
    pub fn new(id: u16, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueSelectPopupFocusColumn {
    Project,
    Issue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueSelectPopupIssue {
    pub project_id: u16,
    pub issue_id: u16,
    pub subject: String,
    pub description: String,
}

impl IssueSelectPopupIssue {
    pub fn new(
        project_id: u16,
        issue_id: u16,
        subject: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            project_id,
            issue_id,
            subject: subject.into(),
            description: description.into(),
        }
    }
}

pub struct IssueSelectPopupWidget<'a> {
    pub projects: &'a [IssueSelectPopupProject],
    pub issues: &'a [IssueSelectPopupIssue],
    pub focused_project_index: usize,
    pub focused_issue_index: usize,
    pub focused_column: IssueSelectPopupFocusColumn,
}

impl<'a> IssueSelectPopupWidget<'a> {
    pub fn new(
        projects: &'a [IssueSelectPopupProject],
        issues: &'a [IssueSelectPopupIssue],
        focused_project_index: usize,
        focused_issue_index: usize,
        focused_column: IssueSelectPopupFocusColumn,
    ) -> Self {
        Self {
            projects,
            issues,
            focused_project_index,
            focused_issue_index,
            focused_column,
        }
    }

    pub fn popup_area(area: Rect) -> Rect {
        let width = area.width.saturating_mul(4) / 5;
        let height = area.height.saturating_mul(4) / 5;

        Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        }
    }

    pub fn line_count(&self, _: u16) -> usize {
        self.projects
            .len()
            .max(self.focused_project_issues().len())
            .max(2)
            .saturating_add(3)
    }

    fn focused_project_issues(&self) -> Vec<&IssueSelectPopupIssue> {
        let Some(project) = self.projects.get(self.focused_project_index) else {
            return Vec::new();
        };

        self.issues
            .iter()
            .filter(|issue| issue.project_id == project.id)
            .collect()
    }
}

impl Widget for IssueSelectPopupWidget<'_> {
    /// クライアント領域に対する描画(したがってClearの責務がある)
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Self::popup_area(area);
        if area.width < 6 || area.height < 4 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default().borders(Borders::ALL).title("Issue選択");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let columns = split_columns(inner);
        let header_style = Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD);
        render_single_line(
            buf,
            columns[0].x,
            inner.y,
            columns[0].width,
            "Project",
            header_style,
        );
        render_single_line(
            buf,
            columns[1].x,
            inner.y,
            columns[1].width,
            "ID",
            header_style,
        );
        render_single_line(
            buf,
            columns[2].x,
            inner.y,
            columns[2].width,
            "Issue",
            header_style,
        );

        let body_height = inner.height.saturating_sub(1) as usize;
        let issues = self.focused_project_issues();
        let focused_issue = issues.get(self.focused_issue_index).copied();

        for (row, project) in self.projects.iter().take(body_height).enumerate() {
            let style = selected_row_style(
                row == self.focused_project_index,
                self.focused_column == IssueSelectPopupFocusColumn::Project,
            );
            render_single_line(
                buf,
                columns[0].x,
                inner.y + 1 + row as u16,
                columns[0].width,
                &project.name,
                style,
            );
        }

        for (row, issue) in issues.iter().take(body_height).enumerate() {
            let style = selected_row_style(
                row == self.focused_issue_index,
                self.focused_column == IssueSelectPopupFocusColumn::Issue,
            );

            render_single_line(
                buf,
                columns[1].x,
                inner.y + 1 + row as u16,
                columns[1].width,
                &issue.issue_id.to_string(),
                style,
            );
        }

        if let Some(issue) = focused_issue {
            let issue_area = Rect {
                x: columns[2].x,
                y: inner.y + 1,
                width: columns[2].width,
                height: inner.height.saturating_sub(1),
            };
            if issue_area.height > 0 {
                let subject = Paragraph::new(issue.subject.as_str()).wrap(Wrap { trim: true });
                let subject_height =
                    (subject.line_count(issue_area.width) as u16).min(issue_area.height);
                subject.render(
                    Rect {
                        height: subject_height,
                        ..issue_area
                    },
                    buf,
                );

                let description_y = issue_area
                    .y
                    .saturating_add(subject_height)
                    .saturating_add(1);
                let issue_area_bottom = issue_area.y.saturating_add(issue_area.height);
                if description_y < issue_area_bottom {
                    let description_area = Rect {
                        x: issue_area.x,
                        y: description_y,
                        width: issue_area.width,
                        height: issue_area_bottom.saturating_sub(description_y),
                    };
                    let description = Paragraph::new(tui_markdown::from_str(&issue.description))
                        .wrap(Wrap { trim: true });
                    description.render(description_area, buf);
                }
            }
        }
    }
}

fn split_columns(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Length(8),
            Constraint::Fill(1),
        ])
        .split(area)
        .to_vec()
}

fn selected_row_style(selected: bool, active: bool) -> Style {
    if selected && active {
        Style::default().bg(FOCUS_BG)
    } else if selected {
        Style::default().bg(SELECTED_BG)
    } else {
        Style::default()
    }
}

fn render_single_line(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    let blank = " ".repeat(width as usize);
    buf.set_line(x, y, &Line::styled(blank, style), width);

    let clipped = text.chars().take(width as usize).collect::<String>();
    buf.set_line(x, y, &Line::styled(clipped, style), width);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;
    use ratatui::style::Modifier;

    fn projects() -> Vec<IssueSelectPopupProject> {
        vec![
            IssueSelectPopupProject::new(1, "redmine-tui"),
            IssueSelectPopupProject::new(2, "backend"),
            IssueSelectPopupProject::new(3, "docs"),
        ]
    }

    fn issues() -> Vec<IssueSelectPopupIssue> {
        vec![
            IssueSelectPopupIssue::new(
                1,
                101,
                "Issue selector popup",
                "3列でproject、ID、subjectとdescriptionを確認できる",
            ),
            IssueSelectPopupIssue::new(
                2,
                204,
                "Long subject that should be clipped by the issue column",
                "Description also needs clipping so the row never wraps unexpectedly",
            ),
            IssueSelectPopupIssue::new(2, 205, "API shape", "projectとissueを別々の一覧で渡す"),
            IssueSelectPopupIssue::new(3, 305, "README更新", "手順と設定例を追加する"),
        ]
    }

    #[test]
    fn snapshot_issue_select_popup_renders_three_columns_with_focused_issue() {
        render_snapshot(
            "issue_select_popup_three_columns_with_focus",
            80,
            24,
            IssueSelectPopupWidget::new(
                &projects(),
                &issues(),
                1,
                0,
                IssueSelectPopupFocusColumn::Issue,
            ),
        );
    }

    #[test]
    fn snapshot_issue_select_popup_renders_empty_issue_columns_when_issue_list_is_empty() {
        render_snapshot(
            "issue_select_popup_empty_issue_list",
            80,
            24,
            IssueSelectPopupWidget::new(&projects(), &[], 1, 0, IssueSelectPopupFocusColumn::Issue),
        );
    }

    #[test]
    fn line_count_includes_header_item_rows_and_borders() {
        let projects = projects();
        let issues = issues();
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            1,
            0,
            IssueSelectPopupFocusColumn::Project,
        );

        assert_eq!(widget.line_count(80), 6);
    }

    #[test]
    fn render_formats_issue_preview_body_as_markdown() {
        let projects = vec![IssueSelectPopupProject::new(1, "redmine-tui")];
        let issues = vec![IssueSelectPopupIssue::new(
            1,
            101,
            "Markdown preview",
            "Preview has **bold** text",
        )];
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            0,
            0,
            IssueSelectPopupFocusColumn::Issue,
        );
        let area = Rect::new(0, 0, 80, 20);
        let mut buffer = Buffer::empty(area);

        widget.render(area, &mut buffer);

        let rendered = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!rendered.contains("**bold**"));

        let bold_cell = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .find(|&(x, y)| buffer[(x, y)].symbol() == "b")
            .expect("bold text should be rendered");
        assert!(buffer[bold_cell].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn render_wraps_subject_and_leaves_blank_line_before_description() {
        let projects = vec![IssueSelectPopupProject::new(1, "redmine-tui")];
        let issues = vec![IssueSelectPopupIssue::new(
            1,
            101,
            "Subject words that must wrap onto another preview line",
            "Description starts after blank line",
        )];
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            0,
            0,
            IssueSelectPopupFocusColumn::Issue,
        );
        let area = Rect::new(0, 0, 80, 20);
        let mut buffer = Buffer::empty(area);

        widget.render(area, &mut buffer);

        let issue_column = issue_column(area);
        assert_eq!(
            line_text(&buffer, issue_column, issue_column.y + 1).trim_end(),
            "Subject words that must wrap onto"
        );
        assert_eq!(
            line_text(&buffer, issue_column, issue_column.y + 2).trim_end(),
            "another preview line"
        );
        assert!(
            line_text(&buffer, issue_column, issue_column.y + 3)
                .trim()
                .is_empty()
        );
        assert_eq!(
            line_text(&buffer, issue_column, issue_column.y + 4).trim_end(),
            "Description starts after blank line"
        );
    }

    fn issue_column(area: Rect) -> Rect {
        let area = IssueSelectPopupWidget::popup_area(area);
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        split_columns(inner)[2]
    }

    fn line_text(buffer: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.x + area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }
}
