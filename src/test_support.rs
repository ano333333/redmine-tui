use chrono::{DateTime, Local};
use insta::assert_snapshot;
use ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer, widgets::Widget};

use crate::entities::{
    Category, Issue, IssueAggregate, IssueStatus, Priority, Project, TargetVersion,
    TimeEntityActivity, Tracker, User,
};
use crate::stores::{Action, Dispatcher, Store};
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId,
    TimeEntityActivityId, TrackerId, UserId,
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

pub fn sync_sample_masters(store: &mut Store) {
    for action in sample_master_actions() {
        store.consume_action(action);
    }
}

pub fn dispatch_sample_masters(dispatcher: &mut Dispatcher) {
    for action in sample_master_actions() {
        dispatcher.dispatch(action);
    }
}

fn sample_master_actions() -> Vec<Action> {
    vec![
        Action::SyncUsers {
            users: sample_users(),
        },
        Action::SyncIssueStatuses {
            issue_statuses: sample_issue_statuses(),
        },
        Action::SyncPriorities {
            priorities: sample_priorities(),
        },
        Action::SyncProjects {
            projects: sample_projects(),
        },
        Action::SyncTrackers {
            trackers: sample_trackers(),
        },
        Action::SyncTargetVersions {
            target_versions: sample_target_versions(),
        },
        Action::SyncCategories {
            categories: sample_categories(),
        },
        Action::SyncTimeEntityActivities {
            time_entity_activities: sample_time_entity_activities(),
        },
    ]
}

pub fn sample_users() -> Vec<User> {
    [(1001, "user1"), (1002, "user2")]
        .map(|(id, name)| User {
            id: UserId::new(id),
            name: name.to_string(),
        })
        .into()
}

pub fn sample_issue_statuses() -> Vec<IssueStatus> {
    [
        (1, "新規(new)", false),
        (2, "割り当て(assigned)", false),
        (3, "進行中(accepted)", false),
        (4, "レビュー(review)", false),
        (5, "完了(closed)", true),
        (6, "改修確認待ち", false),
    ]
    .map(|(id, name, is_closed)| IssueStatus {
        id: IssueStatusId::new(id),
        name: name.to_string(),
        is_closed,
    })
    .into()
}

pub fn sample_priorities() -> Vec<Priority> {
    [(1, "major"), (2, "minor"), (3, "critical"), (4, "blocker")]
        .map(|(id, name)| Priority {
            id: PriorityId::new(id),
            name: name.to_string(),
        })
        .into()
}

pub fn sample_projects() -> Vec<Project> {
    [(1, "Sample Project"), (2, "Sample Project 2")]
        .map(|(id, name)| Project {
            id: ProjectId::new(id),
            name: name.to_string(),
        })
        .into()
}

pub fn sample_trackers() -> Vec<Tracker> {
    [(1, "Bug"), (2, "Feature"), (3, "Support")]
        .map(|(id, name)| Tracker {
            id: TrackerId::new(id),
            name: name.to_string(),
        })
        .into()
}

pub fn sample_target_versions() -> Vec<TargetVersion> {
    vec![TargetVersion {
        id: TargetVersionId::new(1),
        name: "v1.2.3".to_string(),
        project_id: ProjectId::new(1),
    }]
}

pub fn sample_categories() -> Vec<Category> {
    vec![Category {
        id: CategoryId::new(1),
        name: "category1".to_string(),
        project_id: ProjectId::new(1),
    }]
}

pub fn sample_time_entity_activities() -> Vec<TimeEntityActivity> {
    [(1, "設計", true), (2, "実装", false), (3, "検証", false)]
        .map(|(id, name, is_default)| {
            TimeEntityActivity::new(TimeEntityActivityId::new(id), name, is_default)
        })
        .into()
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
        author_id: UserId::new(1),
        created_on: local_datetime("2026-01-10T00:00:00+09:00"),
        updated_on: local_datetime("2026-01-15T00:00:00+09:00"),
        tracker_id: 1.into(),
        priority_id: PriorityId::new(1),
        assigned_to_id: person_in_charge_id.map(UserId::new),
        target_version_id: Some(TargetVersionId::new(1)),
        start_date: start_date.map(local_datetime),
        due_date: due.map(local_datetime),
        done_ratio: progress,
        estimated_hours: Some(8.0),
        total_spent_hours: Some(3.5),
        category_id: Some(CategoryId::new(1)),
        child_ids: vec![],
    }
}
