#![cfg(test)]

use chrono::{DateTime, Local};
use insta::assert_snapshot;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    widgets::Widget,
};

use crate::entities::Issue;

pub fn local_datetime(input: &str) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(input)
        .unwrap()
        .with_timezone(&Local)
}

pub fn render_snapshot<W>(name: &str, width: u16, height: u16, widget: W)
where
    W: Widget,
{
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| frame.render_widget(widget, frame.area()))
        .unwrap();
    assert_snapshot!(name, describe_buffer(terminal.backend().buffer()));
}

fn describe_buffer(buffer: &Buffer) -> String {
    let mut lines = Vec::with_capacity(buffer.area.height as usize + 1);
    lines.push(format!("area={:?}", buffer.area));

    for y in 0..buffer.area.height {
        let mut text = String::with_capacity(buffer.area.width as usize);
        let mut styled_cells = Vec::new();

        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            text.push_str(cell.symbol());

            if cell != &ratatui::buffer::Cell::EMPTY {
                styled_cells.push(format!(
                    "{x}:\"{}\" fg={:?} bg={:?} mod={:?}",
                    cell.symbol(),
                    cell.fg,
                    cell.bg,
                    cell.modifier
                ));
            }
        }

        if styled_cells.is_empty() {
            lines.push(format!("{y:02}: \"{text}\""));
        } else {
            lines.push(format!(
                "{y:02}: \"{text}\" | {}",
                styled_cells.join(", ")
            ));
        }
    }

    lines.join("\n")
}

pub fn sample_issue(
    id: u16,
    title: &str,
    status: &str,
    person_in_charge: Option<&str>,
    start_date: Option<&str>,
    due: Option<&str>,
    progress: u16,
) -> Issue {
    Issue {
        id,
        title: title.to_string(),
        creator: "alice".to_string(),
        appended_at: local_datetime("2026-01-10T00:00:00+09:00"),
        updated_at: local_datetime("2026-01-15T00:00:00+09:00"),
        status: status.to_string(),
        priority: "High".to_string(),
        person_in_charge: person_in_charge.map(str::to_string),
        target_version: Some("v1.2.3".to_string()),
        start_date: start_date.map(local_datetime),
        due: due.map(local_datetime),
        progress,
        planned_hours: Some(8),
        resolve_way: Some("Fixed".to_string()),
        component: "UI".to_string(),
        tags: vec!["frontend".to_string(), "urgent".to_string()],
        body: "body".to_string(),
        child_ids: vec![],
        journal_ids: vec![],
    }
}
