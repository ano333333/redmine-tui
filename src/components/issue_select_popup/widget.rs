use std::cmp::min;
use std::hash::{DefaultHasher, Hash, Hasher};

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use crate::vos::{IssueId, ProjectId};

const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const SELECTED_BG: Color = Color::Rgb(0x22, 0x22, 0x22);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueSelectPopupProject {
    pub id: ProjectId,
    pub name: String,
}

impl IssueSelectPopupProject {
    pub fn new(id: impl Into<ProjectId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
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
    pub project_id: ProjectId,
    pub issue_id: IssueId,
    pub subject: String,
}

impl IssueSelectPopupIssue {
    pub fn new(
        project_id: impl Into<ProjectId>,
        issue_id: impl Into<IssueId>,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            issue_id: issue_id.into(),
            subject: subject.into(),
        }
    }
}

pub struct IssueSelectPopupWidget<'a> {
    pub projects: Vec<&'a IssueSelectPopupProject>,
    pub issues: Vec<&'a IssueSelectPopupIssue>,
    pub focused_project_index: usize,
    pub focused_issue_index: usize,
    pub focused_column: IssueSelectPopupFocusColumn,
    state: &'a IssueSelectPopupWidgetState,
}

pub struct IssueSelectPopupWidgetState {
    subject_buffer: Buffer,
    description_buffer: Buffer,
    preview_hash: Option<u64>,
    preview_generation: u64,
}

impl IssueSelectPopupWidgetState {
    pub fn new() -> Self {
        Self {
            subject_buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
            description_buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
            preview_hash: None,
            preview_generation: 0,
        }
    }

    pub fn update(&mut self, width: u16, issue: &IssueSelectPopupIssue, description: &str) {
        let hash = preview_hash(width, issue, description);
        if self.preview_hash == Some(hash) {
            return;
        }

        self.subject_buffer = render_plain_text_in_buffer(width, &issue.subject);
        self.description_buffer = render_markdown_in_buffer(width, description);
        self.preview_hash = Some(hash);
        self.preview_generation = self.preview_generation.saturating_add(1);
    }

    fn render_preview(&self, area: Rect, buf: &mut Buffer) {
        let subject_height = self.subject_buffer.area.height.min(area.height);
        copy_buffer(&self.subject_buffer, area, buf, subject_height);

        let description_y = area.y.saturating_add(subject_height).saturating_add(1);
        let area_bottom = area.y.saturating_add(area.height);
        if description_y >= area_bottom {
            return;
        }

        let description_area = Rect {
            x: area.x,
            y: description_y,
            width: area.width,
            height: area_bottom.saturating_sub(description_y),
        };
        let description_height = self
            .description_buffer
            .area
            .height
            .min(description_area.height);
        copy_buffer(
            &self.description_buffer,
            description_area,
            buf,
            description_height,
        );
    }
}

impl<'a> IssueSelectPopupWidget<'a> {
    pub fn new(
        projects: impl IntoIterator<Item = &'a IssueSelectPopupProject>,
        issues: impl IntoIterator<Item = &'a IssueSelectPopupIssue>,
        focused_project_index: usize,
        focused_issue_index: usize,
        focused_column: IssueSelectPopupFocusColumn,
        state: &'a IssueSelectPopupWidgetState,
        _description: &str,
    ) -> Self {
        Self {
            projects: projects.into_iter().collect(),
            issues: issues.into_iter().collect(),
            focused_project_index,
            focused_issue_index,
            focused_column,
            state,
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

    /// クライアント領域から、issueのプレビューを描画するカラムの幅を求める。
    /// IssueSelectPopupWidgetStateのキャッシュ更新に使う。
    pub fn preview_width(area: Rect) -> u16 {
        let area = Self::popup_area(area);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        if inner.width == 0 || inner.height == 0 {
            return 0;
        }

        split_columns(inner)[2].width
    }

    pub fn line_count(&self, _: u16) -> usize {
        self.projects
            .len()
            .max(self.issues.len())
            .max(2)
            .saturating_add(3)
    }
}

impl Widget for IssueSelectPopupWidget<'_> {
    /// クライアント領域に対する描画(したがってClearの責務がある)
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_issue_select_popup(self, area, buf);
    }
}

fn render_issue_select_popup(widget: IssueSelectPopupWidget<'_>, area: Rect, buf: &mut Buffer) {
    let area = IssueSelectPopupWidget::popup_area(area);
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
    let focused_issue = widget.issues.get(widget.focused_issue_index).copied();

    for (row, project) in widget.projects.iter().take(body_height).enumerate() {
        let active_project = widget.focused_column == IssueSelectPopupFocusColumn::Project
            || (widget.focused_column == IssueSelectPopupFocusColumn::Issue
                && focused_issue.is_some_and(|issue| issue.project_id == project.id));
        let style = selected_row_style(row == widget.focused_project_index, active_project);
        render_single_line(
            buf,
            columns[0].x,
            inner.y + 1 + row as u16,
            columns[0].width,
            &project.name,
            style,
        );
    }

    for (row, issue) in widget.issues.iter().take(body_height).enumerate() {
        let style = selected_row_style(
            row == widget.focused_issue_index,
            widget.focused_column == IssueSelectPopupFocusColumn::Issue,
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

    if focused_issue.is_some() {
        let issue_area = Rect {
            x: columns[2].x,
            y: inner.y + 1,
            width: columns[2].width,
            height: inner.height.saturating_sub(1),
        };
        if issue_area.height > 0 {
            widget.state.render_preview(issue_area, buf);
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

fn preview_hash(width: u16, issue: &IssueSelectPopupIssue, description: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    width.hash(&mut hasher);
    issue.subject.hash(&mut hasher);
    description.hash(&mut hasher);
    hasher.finish()
}

fn render_plain_text_in_buffer(width: u16, text: &str) -> Buffer {
    if width == 0 {
        return Buffer::empty(Rect::new(0, 0, 0, 0));
    }

    let paragraph = Paragraph::new(text.to_string()).wrap(Wrap { trim: true });
    render_paragraph_in_buffer(paragraph, width)
}

fn render_markdown_in_buffer(width: u16, text: &str) -> Buffer {
    if width == 0 {
        return Buffer::empty(Rect::new(0, 0, 0, 0));
    }

    let paragraph = Paragraph::new(tui_markdown::from_str(text)).wrap(Wrap { trim: true });
    render_paragraph_in_buffer(paragraph, width)
}

fn render_paragraph_in_buffer(paragraph: Paragraph<'_>, width: u16) -> Buffer {
    let area = Rect::new(0, 0, width, paragraph.line_count(width) as u16);
    let mut buffer = Buffer::empty(area);
    paragraph.render(area, &mut buffer);
    buffer
}

fn copy_buffer(src: &Buffer, dst_area: Rect, dst: &mut Buffer, height: u16) {
    let width = min(src.area.width, dst_area.width);

    for y in 0..height {
        for x in 0..width {
            let Some(src_cell) = src.cell((x, y)).cloned() else {
                continue;
            };
            let dst_x = dst_area.x + x;
            let dst_y = dst_area.y + y;
            if let Some(dst_cell) = dst.cell_mut((dst_x, dst_y)) {
                *dst_cell = src_cell;
            }
        }
    }
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
            IssueSelectPopupIssue::new(1, 101, "Issue selector popup"),
            IssueSelectPopupIssue::new(
                2,
                204,
                "Long subject that should be clipped by the issue column",
            ),
            IssueSelectPopupIssue::new(2, 205, "API shape"),
            IssueSelectPopupIssue::new(3, 305, "README更新"),
        ]
    }

    #[test]
    fn snapshot_issue_select_popup_renders_three_columns_with_focused_issue() {
        let projects = projects();
        let issues = issues()
            .into_iter()
            .filter(|issue| issue.project_id == ProjectId::new(2))
            .collect::<Vec<_>>();
        let description =
            "Description also needs clipping so the row never wraps unexpectedly".to_string();
        let mut state = IssueSelectPopupWidgetState::new();
        state.update(
            IssueSelectPopupWidget::preview_width(Rect::new(0, 0, 80, 24)),
            &issues[0],
            &description,
        );
        render_snapshot(
            "issue_select_popup_three_columns_with_focus",
            80,
            24,
            IssueSelectPopupWidget::new(
                &projects,
                &issues,
                1,
                0,
                IssueSelectPopupFocusColumn::Issue,
                &state,
                &description,
            ),
        );
    }

    #[test]
    fn snapshot_issue_select_popup_renders_empty_issue_columns_when_issue_list_is_empty() {
        let projects = projects();
        let state = IssueSelectPopupWidgetState::new();
        render_snapshot(
            "issue_select_popup_empty_issue_list",
            80,
            24,
            IssueSelectPopupWidget::new(
                &projects,
                &Vec::new(),
                1,
                0,
                IssueSelectPopupFocusColumn::Issue,
                &state,
                "",
            ),
        );
    }

    #[test]
    fn line_count_includes_header_item_rows_and_borders() {
        let projects = vec![IssueSelectPopupProject::new(1, "redmine-tui")];
        let issues = vec![
            IssueSelectPopupIssue::new(2, 201, "Displayed issue 1"),
            IssueSelectPopupIssue::new(2, 202, "Displayed issue 2"),
            IssueSelectPopupIssue::new(2, 203, "Displayed issue 3"),
        ];
        let state = IssueSelectPopupWidgetState::new();
        let description = String::new();
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            0,
            0,
            IssueSelectPopupFocusColumn::Project,
            &state,
            &description,
        );

        assert_eq!(widget.line_count(80), 6);
    }

    #[test]
    fn render_highlights_focused_issue_project_when_issue_column_is_active() {
        let projects = projects();
        let issues = issues()
            .into_iter()
            .filter(|issue| issue.project_id == ProjectId::new(2))
            .collect::<Vec<_>>();
        let area = Rect::new(0, 0, 80, 24);
        let project_column = project_column(area);
        let description =
            "Description also needs clipping so the row never wraps unexpectedly".to_string();
        let mut state = IssueSelectPopupWidgetState::new();
        state.update(
            IssueSelectPopupWidget::preview_width(area),
            &issues[0],
            &description,
        );
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            1,
            0,
            IssueSelectPopupFocusColumn::Issue,
            &state,
            &description,
        );
        let mut buffer = Buffer::empty(area);

        Widget::render(widget, area, &mut buffer);

        let project_row = project_column.y + 1 + 1;
        for x in project_column.x..project_column.x + project_column.width {
            assert_eq!(buffer[(x, project_row)].bg, FOCUS_BG);
        }
    }

    #[test]
    fn render_formats_issue_preview_body_as_markdown() {
        let projects = vec![IssueSelectPopupProject::new(1, "redmine-tui")];
        let issues = vec![IssueSelectPopupIssue::new(1, 101, "Markdown preview")];
        let description = "Preview has **bold** text".to_string();
        let area = Rect::new(0, 0, 80, 20);
        let mut state = IssueSelectPopupWidgetState::new();
        state.update(
            IssueSelectPopupWidget::preview_width(area),
            &issues[0],
            &description,
        );
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            0,
            0,
            IssueSelectPopupFocusColumn::Issue,
            &state,
            &description,
        );
        let mut buffer = Buffer::empty(area);

        Widget::render(widget, area, &mut buffer);

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
        )];
        let description = "Description starts after blank line".to_string();
        let area = Rect::new(0, 0, 80, 20);
        let issue_column = issue_column(area);
        let mut state = IssueSelectPopupWidgetState::new();
        state.update(
            IssueSelectPopupWidget::preview_width(area),
            &issues[0],
            &description,
        );
        let widget = IssueSelectPopupWidget::new(
            &projects,
            &issues,
            0,
            0,
            IssueSelectPopupFocusColumn::Issue,
            &state,
            &description,
        );
        let mut buffer = Buffer::empty(area);

        Widget::render(widget, area, &mut buffer);

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

    /// preview_widthはStateのキャッシュ生成に使われるので、実際に描画されるカラム幅と一致している必要がある
    #[test]
    fn preview_width_matches_rendered_issue_column_width() {
        for area in [
            Rect::new(0, 0, 80, 24),
            Rect::new(0, 0, 40, 20),
            Rect::new(4, 2, 100, 30),
        ] {
            assert_eq!(
                IssueSelectPopupWidget::preview_width(area),
                issue_column(area).width,
                "preview width should match the issue column width for {area:?}"
            );
        }
    }

    fn issue_column(area: Rect) -> Rect {
        let area = IssueSelectPopupWidget::popup_area(area);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        split_columns(inner)[2]
    }

    fn project_column(area: Rect) -> Rect {
        let area = IssueSelectPopupWidget::popup_area(area);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        split_columns(inner)[0]
    }

    fn line_text(buffer: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.x + area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }
}
