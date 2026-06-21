#![cfg(test)]

use chrono::{DateTime, Local};
use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, widgets::Widget};

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
            lines.push(format!("{y:02}: \"{text}\" | {}", styled_cells.join(", ")));
        }
    }

    lines.join("\n")
}

pub fn sample_issue(
    id: u16,
    title: &str,
    issue_status_id: u16,
    person_in_charge_id: Option<u16>,
    start_date: Option<&str>,
    due: Option<&str>,
    progress: u16,
) -> Issue {
    Issue {
        id,
        subject: title.to_string(),
        author_id: 1,
        created_on: local_datetime("2026-01-10T00:00:00+09:00"),
        updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
        status_id: issue_status_id,
        priority_id: 1,
        assigned_to_id: person_in_charge_id,
        fixed_version: Some("v1.2.3".to_string()),
        start_date: start_date.map(local_datetime),
        due_date: due.map(local_datetime),
        done_ratio: progress,
        estimated_hours: Some(8),
        resolve_way: Some("Fixed".to_string()),
        component: "UI".to_string(),
        tags: vec!["frontend".to_string(), "urgent".to_string()],
        description: "body".to_string(),
        child_ids: vec![],
        journal_ids: vec![],
    }
}
