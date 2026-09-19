use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::vos::IssueId;
use crate::widgets::gutter::{GUTTER_WIDTH, Gutter, indented_area};
use crate::widgets::theme::{ACCENT, FOCUS_BG, MUTED};

/// ラベル列の表示幅(全角換算)。値はこの位置から始まる。
const LABEL_WIDTH: usize = 14;
/// プロパティの論理的な項目数。FocusStateのLINE_COUNTと一致させる。
const FIELD_COUNT: usize = 15;
/// 2カラムに畳んだときの左カラムの表示行数。
const ROW_COUNT: u16 = FIELD_COUNT.div_ceil(2) as u16;
/// 2カラムに畳んだとき、左右カラムの間に空ける桁数。
const COLUMN_GAP: u16 = 2;
/// 2カラムに畳んで表示できる最小の本文幅。これ未満は1カラムへフォールバックする。
const TWO_COLUMN_MIN_WIDTH: u16 = 64;
/// 縦線を引かない末尾の余白行。次のブロックとの区切りになる。
const GUTTER_TRAILING_LINES: u16 = 1;

pub struct PropertyWidget<'a> {
    id: IssueId,
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
        let focused_index = self.focused_y;
        let fields = self.fields();

        let gutter_area = Rect::new(
            area.x,
            area.y,
            area.width,
            area.height
                .min((self.line_count(area.width) as u16).saturating_sub(GUTTER_TRAILING_LINES)),
        );
        Gutter::line().render(gutter_area, buf);
        let content = indented_area(area);

        if !is_two_column(content.width) {
            render_column(&fields, content, buf, focused_index, 0);
            return;
        }

        let columns = split_columns(content);
        // 左カラムに前半、右カラムに後半を積む。項目の並び順は1カラム時と変えない。
        let split_at = FIELD_COUNT.div_ceil(2);
        render_column(&fields[..split_at], columns[0], buf, focused_index, 0);
        render_column(
            &fields[split_at..],
            columns[1],
            buf,
            focused_index,
            split_at,
        );
    }
}

/// 幅が足りるときだけ2カラムに畳む。
fn is_two_column(width: u16) -> bool {
    width >= TWO_COLUMN_MIN_WIDTH
}

/// 描画領域を左右2カラム + 間の余白に割る。
fn split_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(COLUMN_GAP),
            Constraint::Fill(1),
        ])
        .split(area)
        .iter()
        // 間の余白は捨て、左右カラムだけを返す
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .map(|(_, rect)| *rect)
        .collect()
}

/// 1つのカラムに項目を縦積みし、フォーカス項目だけ背景を塗る。
fn render_column(
    fields: &[PropertyField<'_>],
    area: Rect,
    buf: &mut Buffer,
    focused_index: Option<u16>,
    index_offset: usize,
) {
    for (row, field) in fields.iter().enumerate() {
        let row = row as u16;
        if row >= area.height {
            return;
        }

        let line_area = Rect::new(area.x, area.y + row, area.width, 1);
        Paragraph::new(field.to_line()).render(line_area, buf);

        if focused_index == Some((index_offset + row as usize) as u16) {
            fill_background(buf, line_area);
        }
    }
}

fn fill_background(buf: &mut Buffer, area: Rect) {
    for x in 0..area.width {
        if let Some(cell) = buf.cell_mut((area.x + x, area.y)) {
            cell.set_bg(FOCUS_BG);
        }
    }
}

impl<'a> PropertyWidget<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<IssueId>,
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
            id: id.into(),
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

    /// 値列が始まる桁。FocusStateのカーソル位置と揃える。
    pub const VALUE_X: u16 = LABEL_WIDTH as u16;

    /// 幅が足りるときだけ2カラムに畳む。判定は縦線を除いた本文幅で行う。
    pub fn is_two_column(width: u16) -> bool {
        is_two_column(width.saturating_sub(GUTTER_WIDTH))
    }

    /// 2カラム時に、右カラムが始まる桁(縦線の字下げを含む)。
    pub fn right_column_x(width: u16) -> u16 {
        let content = indented_area(Rect::new(0, 0, width, 1));
        split_columns(content)[1].x
    }

    pub fn line_count(&self, width: u16) -> usize {
        let rows = if Self::is_two_column(width) {
            ROW_COUNT
        } else {
            FIELD_COUNT as u16
        };
        // 最終行は縦線を引かない余白として使う
        (rows + GUTTER_TRAILING_LINES) as usize
    }

    /// 15項目をFocusStateのインデックス順に並べて返す。
    fn fields(&self) -> Vec<PropertyField<'a>> {
        let person = self.person_in_charge.unwrap_or("-");
        let target_version = self.target_version.unwrap_or("-");
        let planned_hours = match self.planned_hours {
            Some(p) => p.to_string(),
            None => "-".to_string(),
        };
        let total_spent_hours = match self.total_spent_hours {
            Some(hours) if hours.fract() == 0.0 => format!("{hours:.0}"),
            Some(hours) => hours.to_string(),
            None => "-".to_string(),
        };

        vec![
            PropertyField::text("作成者", self.author),
            PropertyField::text("作成日", format_date(&Some(self.created_on))),
            PropertyField::text("更新日", format_date(&Some(self.updated_on))),
            PropertyField::styled("ステータス", self.status, Style::default().blue().bold()),
            PropertyField::text("トラッカー", self.tracker),
            PropertyField::text("優先度", self.priority),
            PropertyField::text("プロジェクト", self.project),
            PropertyField::styled("担当者", person, Style::default().blue()),
            PropertyField::text("対象バージョン", target_version),
            PropertyField::text("開始日", format_date(&self.start_date)),
            PropertyField::text("期日", format_date(&self.due)),
            PropertyField::progress(self.progress),
            PropertyField::text("予定工数", format!("{planned_hours} h")),
            PropertyField::text("実績工数", format!("{total_spent_hours} h")),
            PropertyField::text("カテゴリー", self.category),
        ]
    }
}

/// 表示用に解決済みの1項目。
///
/// 値が単なる文字列のものと、進捗率のようにSpanを組み立てるものがあるので、
/// 行の作り方ごと保持する。
enum PropertyField<'a> {
    Text {
        label: &'a str,
        value: String,
        style: Style,
    },
    Progress {
        progress: u16,
    },
}

impl<'a> PropertyField<'a> {
    fn text(label: &'a str, value: impl Into<String>) -> Self {
        Self::Text {
            label,
            value: value.into(),
            style: Style::default(),
        }
    }

    fn styled(label: &'a str, value: impl Into<String>, style: Style) -> Self {
        Self::Text {
            label,
            value: value.into(),
            style,
        }
    }

    fn progress(progress: u16) -> Self {
        Self::Progress { progress }
    }

    fn to_line(&self) -> Line<'_> {
        match self {
            Self::Text {
                label,
                value,
                style,
            } => Line::from(vec![
                Span::from(pad_label(label)).fg(MUTED),
                Span::from(value.as_str()).style(*style),
            ]),
            Self::Progress { progress } => progress_line(*progress),
        }
    }
}

/// ラベルを表示幅 LABEL_WIDTH まで空白で埋める。全角文字は2桁として数える。
fn pad_label(label: &str) -> String {
    let display_width: usize = label
        .chars()
        .map(|c| if c.is_ascii() { 1 } else { 2 })
        .sum();
    format!(
        "{}{}",
        label,
        " ".repeat(LABEL_WIDTH.saturating_sub(display_width))
    )
}

fn format_date(date_opt: &Option<DateTime<Local>>) -> String {
    match date_opt {
        Some(d) => d.format("%Y/%m/%d").to_string(),
        None => "-".to_string(),
    }
}

/// 進捗率を数値と塗り分けたバーで示す。行の高さは増やさない。
fn progress_line<'a>(progress: u16) -> Line<'a> {
    const BAR_LEN: u16 = 12;
    let filled = (progress.min(100) * BAR_LEN / 100) as usize;
    Line::from(vec![
        Span::from(pad_label("進捗率")).fg(MUTED),
        Span::from(format!("{progress:>3}%  ")),
        Span::from("━".repeat(filled)).fg(ACCENT),
        Span::from("─".repeat(BAR_LEN as usize - filled)).fg(MUTED),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, render_snapshot};

    #[test]
    fn snapshot_property_two_column_wide() {
        let status = "In Progress".to_string();
        let tracker = "Bug".to_string();
        let priority = "critical".to_string();
        let project = "Sample Project".to_string();
        let person_in_charge = Some("alice".to_string());
        let target_version = Some("2026 Spring".to_string());
        let category = "Admin UI".to_string();
        let widget = PropertyWidget::new(
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
        );
        let line_count = widget.line_count(100) as u16;
        // 15項目を8行に畳み、末尾に余白1行。
        assert_eq!(line_count, 9);
        render_snapshot("property_two_column_wide", 100, line_count, widget);
    }

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
        // 幅40/22はどちらも2カラムに畳まないので、15項目 + 末尾の余白1行。
        assert_eq!(widget.line_count(40), 16);
        assert_eq!(widget.line_count(22), 16);
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
