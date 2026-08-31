use chrono::{DateTime, Local};
use insta::assert_snapshot;
use ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer, widgets::Widget};

use crate::entities::{Issue, IssueAggregate};
use crate::libs::yaml::{
    parse_categories_yaml, parse_issue_statuses_yaml, parse_priorities_yaml, parse_projects_yaml,
    parse_target_versions_yaml, parse_time_entity_activities_yaml, parse_trackers_yaml,
    parse_users_yaml,
};
use crate::stores::{Action, Dispatcher, Store};
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, UserId,
};

pub fn local_datetime(input: &str) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(input)
        .unwrap()
        .with_timezone(&Local)
}

pub fn render_snapshot<W>(name: &str, width: u16, height: u16, widget: W)
where
    W: Widget,
{
    render_frame_snapshot(name, width, height, |frame| {
        frame.render_widget(widget, frame.area())
    });
}

/// Widgetを直接渡せないComponent(Frame越しに描画するもの)向けのスナップショット
pub fn render_frame_snapshot<F>(name: &str, width: u16, height: u16, render: F)
where
    F: FnOnce(&mut Frame<'_>),
{
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(render).unwrap();
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

pub fn sync_fixture_entities(store: &mut Store) {
    for action in fixture_entity_actions() {
        store.consume_action(action);
    }
}

pub fn dispatch_fixture_entity_actions(dispatcher: &mut Dispatcher) {
    for action in fixture_entity_actions() {
        dispatcher.dispatch(action);
    }
}

fn fixture_entity_actions() -> Vec<Action> {
    vec![
        Action::SyncUsers {
            users: parse_users_yaml().into_values().collect(),
        },
        Action::SyncIssueStatuses {
            issue_statuses: parse_issue_statuses_yaml().into_values().collect(),
        },
        Action::SyncPriorities {
            priorities: parse_priorities_yaml().into_values().collect(),
        },
        Action::SyncProjects {
            projects: parse_projects_yaml().into_values().collect(),
        },
        Action::SyncTrackers {
            trackers: parse_trackers_yaml().into_values().collect(),
        },
        Action::SyncTargetVersions {
            target_versions: parse_target_versions_yaml().into_values().collect(),
        },
        Action::SyncCategories {
            categories: parse_categories_yaml().into_values().collect(),
        },
        Action::SyncTimeEntityActivities {
            time_entity_activities: parse_time_entity_activities_yaml().into_values().collect(),
        },
    ]
}

pub fn sample_issue_aggregate(
    id: u16,
    title: &str,
    issue_status_id: IssueStatusId,
    person_in_charge_id: Option<u16>,
    start_date: Option<&str>,
    due: Option<&str>,
    progress: u16,
) -> IssueAggregate {
    IssueAggregate {
        issue: Issue {
            id: IssueId::new(id),
            project_id: ProjectId::new(1),
            subject: title.to_string(),
            description: "body".to_string(),
            status_id: issue_status_id,
        },
        id: IssueId::new(id),
        subject: title.to_string(),
        author_id: UserId::new(1),
        created_on: local_datetime("2026-01-10T00:00:00+09:00"),
        updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
        project_id: ProjectId::new(1),
        tracker_id: 1.into(),
        status_id: issue_status_id,
        priority_id: PriorityId::new(1),
        assigned_to_id: person_in_charge_id.map(UserId::new),
        target_version_id: Some(TargetVersionId::new(1)),
        start_date: start_date.map(local_datetime),
        due_date: due.map(local_datetime),
        done_ratio: progress,
        estimated_hours: Some(8),
        total_spent_hours: Some(3.5),
        category_id: Some(CategoryId::new(1)),
        description: "body".to_string(),
        child_ids: vec![],
        journal_ids: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::sample_issue_aggregate;
    use crate::vos::IssueStatusId;

    #[test]
    fn sample_issue_aggregate_keeps_lightweight_issue_fields_in_sync() {
        let aggregate = sample_issue_aggregate(
            42,
            "Fix login",
            IssueStatusId::new(3),
            Some(1001),
            None,
            None,
            30,
        );

        assert_eq!(aggregate.issue.id, aggregate.id);
        assert_eq!(aggregate.issue.project_id, aggregate.project_id);
        assert_eq!(aggregate.issue.subject, aggregate.subject);
        assert_eq!(aggregate.issue.description, aggregate.description);
        assert_eq!(aggregate.issue.status_id, aggregate.status_id);
    }
}
