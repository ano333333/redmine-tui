use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);
const MESSAGE: &str = "編集内容とサーバー内容が競合しています。採用する方を選択してください。";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IssuePropertyConflictFocus {
    After,
    Server,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IssuePropertyConflictButton {
    Cancel,
    Continue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IssuePropertyConflictValueFormat {
    PlainText,
    Markdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuePropertyConflictRow {
    property_name: String,
    before: String,
    after: String,
    server: String,
    focused_choice: IssuePropertyConflictFocus,
    value_format: IssuePropertyConflictValueFormat,
}

impl IssuePropertyConflictRow {
    /// プレーンテキストとして描画する競合行を作成する。
    pub fn new(
        property_name: impl Into<String>,
        before: impl Into<String>,
        after: impl Into<String>,
        server: impl Into<String>,
        focused_choice: IssuePropertyConflictFocus,
    ) -> Self {
        Self::new_with_format(
            property_name,
            before,
            after,
            server,
            focused_choice,
            IssuePropertyConflictValueFormat::PlainText,
        )
    }

    /// Markdownとして描画する競合行を作成する。
    pub fn new_markdown(
        property_name: impl Into<String>,
        before: impl Into<String>,
        after: impl Into<String>,
        server: impl Into<String>,
        focused_choice: IssuePropertyConflictFocus,
    ) -> Self {
        Self::new_with_format(
            property_name,
            before,
            after,
            server,
            focused_choice,
            IssuePropertyConflictValueFormat::Markdown,
        )
    }

    fn new_with_format(
        property_name: impl Into<String>,
        before: impl Into<String>,
        after: impl Into<String>,
        server: impl Into<String>,
        focused_choice: IssuePropertyConflictFocus,
        value_format: IssuePropertyConflictValueFormat,
    ) -> Self {
        Self {
            property_name: property_name.into(),
            before: before.into(),
            after: after.into(),
            server: server.into(),
            focused_choice,
            value_format,
        }
    }

    /// 現在フォーカス表示されている採用元を返す。
    pub fn focused_choice(&self) -> IssuePropertyConflictFocus {
        self.focused_choice
    }

    /// フォーカス表示する採用元を差し替えた行を返す。
    pub fn with_focused_choice(mut self, focused_choice: IssuePropertyConflictFocus) -> Self {
        self.focused_choice = focused_choice;
        self
    }
}

pub struct IssuePropertyConflictWidget<'a> {
    rows: &'a [IssuePropertyConflictRow],
    focused_button: Option<IssuePropertyConflictButton>,
}

impl<'a> IssuePropertyConflictWidget<'a> {
    /// 競合行一覧からWidgetを作成する。
    pub fn new(rows: &'a [IssuePropertyConflictRow]) -> Self {
        Self {
            rows,
            focused_button: None,
        }
    }

    /// フォーカス表示するボタンを指定したWidgetを返す。
    pub fn with_focused_button(
        mut self,
        focused_button: Option<IssuePropertyConflictButton>,
    ) -> Self {
        self.focused_button = focused_button;
        self
    }

    /// 指定幅で全体を描画するために必要な行数を返す。
    pub fn line_count(&self, width: u16) -> usize {
        if width == 0 {
            return 0;
        }

        let message_line_count = Paragraph::new(MESSAGE).line_count(width);
        let row_line_count = self
            .rows
            .iter()
            .map(|row| row_line_count(row, width))
            .sum::<usize>();

        message_line_count
            .saturating_add(1)
            .saturating_add(1)
            .saturating_add(row_line_count)
            .saturating_add(1)
            .saturating_add(3)
    }

    /// 指定セルの仮想バッファ上のカーソル位置を返す。
    pub fn cursor_position_for_cell(
        &self,
        row_index: usize,
        column: IssuePropertyConflictFocus,
        width: u16,
    ) -> Position {
        let row_y = self.row_y(row_index, width);
        Position {
            x: choice_column_x(column, width),
            y: row_y,
        }
    }

    /// 指定ボタンの仮想バッファ上のカーソル位置を返す。
    pub fn cursor_position_for_button(
        &self,
        button: IssuePropertyConflictButton,
        width: u16,
    ) -> Position {
        Position {
            x: button_x(button, width),
            y: self.line_count(width).saturating_sub(2) as u16,
        }
    }

    fn row_y(&self, row_index: usize, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }

        let rows_before = self
            .rows
            .iter()
            .take(row_index)
            .map(|row| row_line_count(row, width))
            .sum::<usize>();
        Paragraph::new(MESSAGE)
            .line_count(width)
            .saturating_add(1)
            .saturating_add(1)
            .saturating_add(rows_before) as u16
    }
}

impl Widget for IssuePropertyConflictWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let message_line_count = Paragraph::new(MESSAGE).line_count(area.width) as u16;
        let mut row_constraints = Vec::with_capacity(self.rows.len() + 5);
        row_constraints.push(Constraint::Length(message_line_count));
        row_constraints.push(Constraint::Length(1));
        row_constraints.push(Constraint::Length(1));
        row_constraints.extend(
            self.rows
                .iter()
                .map(|row| Constraint::Length(row_line_count(row, area.width) as u16)),
        );
        row_constraints.push(Constraint::Length(1));
        row_constraints.push(Constraint::Length(3));
        let row_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(area);

        Paragraph::new(MESSAGE)
            .wrap(Wrap { trim: false })
            .render(row_areas[0], buf);

        render_header(row_areas[2], buf);

        for (row, area) in self.rows.iter().zip(row_areas.iter().skip(3)) {
            render_row(row, *area, buf);
        }

        if let Some(button_area) = row_areas.last() {
            render_buttons(*button_area, buf, self.focused_button);
        }
    }
}

fn render_header(area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let cols = column_areas(area);
    render_header_cell("プロパティ名", cols[0], buf);
    render_header_cell("編集前の値", cols[1], buf);
    render_header_cell("編集後の値", cols[2], buf);
    render_header_cell("サーバーの値", cols[3], buf);
}

fn render_header_cell(text: &'static str, area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let area = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    if area.width == 0 {
        return;
    }

    Paragraph::new(Line::styled(
        text,
        Style::default().add_modifier(Modifier::BOLD),
    ))
    .wrap(Wrap { trim: false })
    .render(area, buf);
}

fn render_row(row: &IssuePropertyConflictRow, area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let cols = column_areas(area);

    render_cell(
        &row.property_name,
        cols[0],
        buf,
        false,
        IssuePropertyConflictValueFormat::PlainText,
    );
    render_cell(&row.before, cols[1], buf, false, row.value_format);
    render_cell(
        &row.after,
        cols[2],
        buf,
        row.focused_choice == IssuePropertyConflictFocus::After,
        row.value_format,
    );
    render_cell(
        &row.server,
        cols[3],
        buf,
        row.focused_choice == IssuePropertyConflictFocus::Server,
        row.value_format,
    );
}

fn column_areas(area: Rect) -> [Rect; 4] {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(1),
            Constraint::Percentage(25),
            Constraint::Length(1),
            Constraint::Percentage(25),
            Constraint::Length(1),
            Constraint::Percentage(25),
        ])
        .split(area);
    [cols[0], cols[2], cols[4], cols[6]]
}

fn render_cell(
    text: &str,
    area: Rect,
    buf: &mut Buffer,
    focused: bool,
    value_format: IssuePropertyConflictValueFormat,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let style = focus_style(focused);
    for y in area.y..area.y.saturating_add(area.height) {
        let blank = " ".repeat(area.width as usize);
        buf.set_line(area.x, y, &Line::styled(blank, style), area.width);
    }

    match value_format {
        IssuePropertyConflictValueFormat::PlainText => {
            Paragraph::new(text)
                .style(style)
                .wrap(Wrap { trim: false })
                .render(area, buf);
        }
        IssuePropertyConflictValueFormat::Markdown => {
            Paragraph::new(tui_markdown::from_str(text))
                .style(style)
                .wrap(Wrap { trim: false })
                .render(area, buf);
        }
    }
}

fn render_buttons(
    area: Rect,
    buf: &mut Buffer,
    focused_button: Option<IssuePropertyConflictButton>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(1),
            Constraint::Length(10),
        ])
        .split(area);

    render_button(
        "キャンセル",
        cols[1],
        buf,
        focused_button == Some(IssuePropertyConflictButton::Cancel),
    );
    render_button(
        "続行",
        cols[3],
        buf,
        focused_button == Some(IssuePropertyConflictButton::Continue),
    );
}

fn render_button(text: &'static str, area: Rect, buf: &mut Buffer, focused: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    Paragraph::new(Line::from(text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(button_border_style(focused)),
        )
        .render(area, buf);
}

fn row_line_count(row: &IssuePropertyConflictRow, width: u16) -> usize {
    let cols = column_areas(Rect::new(0, 0, width, 1));
    [
        cell_line_count(
            &row.property_name,
            cols[0].width,
            IssuePropertyConflictValueFormat::PlainText,
        ),
        cell_line_count(&row.before, cols[1].width, row.value_format),
        cell_line_count(&row.after, cols[2].width, row.value_format),
        cell_line_count(&row.server, cols[3].width, row.value_format),
    ]
    .into_iter()
    .max()
    .unwrap_or(1)
}

fn cell_line_count(
    text: &str,
    width: u16,
    value_format: IssuePropertyConflictValueFormat,
) -> usize {
    if width == 0 {
        return 0;
    }

    match value_format {
        IssuePropertyConflictValueFormat::PlainText => Paragraph::new(text).line_count(width),
        IssuePropertyConflictValueFormat::Markdown => {
            Paragraph::new(tui_markdown::from_str(text)).line_count(width)
        }
    }
}

fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default().bg(FOCUS_BG)
    } else {
        Style::default()
    }
}

fn button_border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default()
    }
}

fn choice_column_x(column: IssuePropertyConflictFocus, width: u16) -> u16 {
    let cols = column_areas(Rect::new(0, 0, width, 1));
    match column {
        IssuePropertyConflictFocus::After => cols[2].x,
        IssuePropertyConflictFocus::Server => cols[3].x,
    }
}

fn button_x(button: IssuePropertyConflictButton, width: u16) -> u16 {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(1),
            Constraint::Length(10),
        ])
        .split(Rect::new(0, 0, width, 3));

    match button {
        IssuePropertyConflictButton::Cancel => cols[1].x.saturating_add(1),
        IssuePropertyConflictButton::Continue => cols[3].x.saturating_add(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_issue_property_conflict_widget_renders_full_area_with_message_rows_and_buttons() {
        let rows = vec![
            IssuePropertyConflictRow::new(
                "ステータス",
                "New",
                "In Progress after local editing",
                "Closed by server side update",
                IssuePropertyConflictFocus::After,
            ),
            IssuePropertyConflictRow::new(
                "期日",
                "2026-02-01",
                "2026-02-15",
                "2026-02-20",
                IssuePropertyConflictFocus::Server,
            ),
        ];

        render_snapshot(
            "issue_property_conflict_widget_full_area_with_message_rows_and_buttons",
            120,
            28,
            IssuePropertyConflictWidget::new(&rows),
        );
    }

    #[test]
    fn line_count_includes_message_header_rows_and_buttons() {
        let rows = vec![
            IssuePropertyConflictRow::new(
                "ステータス",
                "New",
                "In Progress",
                "Closed",
                IssuePropertyConflictFocus::After,
            ),
            IssuePropertyConflictRow::new(
                "期日",
                "2026-02-01",
                "2026-02-15",
                "2026-02-20",
                IssuePropertyConflictFocus::Server,
            ),
        ];
        let widget = IssuePropertyConflictWidget::new(&rows);

        assert_eq!(widget.line_count(80), 9);
    }

    #[test]
    fn snapshot_issue_property_conflict_widget_renders_description_row_as_markdown() {
        let rows = vec![IssuePropertyConflictRow::new_markdown(
            "説明",
            "# Before\n\n- old item",
            "# After\n\n- local item",
            "# Server\n\n- remote item",
            IssuePropertyConflictFocus::Server,
        )];

        render_snapshot(
            "issue_property_conflict_widget_description_markdown_row",
            120,
            28,
            IssuePropertyConflictWidget::new(&rows),
        );
    }
}
