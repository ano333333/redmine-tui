use chrono::{DateTime, Local, NaiveDate, TimeZone};
use insta::assert_snapshot;
use ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer, widgets::Widget};

use crate::entities::{
    Category, Issue, IssueAggregate, IssueChild, IssueStatus, Journal, Priority, Project,
    TargetVersion, TimeEntityActivity, Tracker, User,
};
use crate::stores::{Action, Dispatcher, Store};
use crate::usecases::UsecaseTask;
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId, PriorityId,
    ProjectId, TargetVersionId, TimeEntityActivityId, TrackerId, UserId,
};

/// 起動したUsecaseを完了まで進め、完了Actionを返す。起動しなかった場合はpanicする。
pub async fn complete_usecase(task: Option<UsecaseTask>) -> Vec<Action> {
    task.expect("usecase should start").await.actions
}

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
        parent_id: None,
        journals: vec![],
    }
}

pub fn local_date(year: i32, month: u32, day: u32) -> DateTime<Local> {
    let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .unwrap()
}

pub const SAMPLE_MARKDOWN: &str = r#"### h3

#### h4

##### h5

normal text

*italic text*

**bold text**

1. numbered list 1
1. numbered list 2
1. numbered list 3
  1. inner numbered list 1
  1. inner numbered list 2
  1. inner numbered list 3

- itemized list 1
- itemized list 2
- itemized list 3
  - itemized list 1
  - itemized list 2
  - itemized list 3

~~canceled text~~

`code`

```
code block
```

> citation"#;

pub fn sample_open_child_issue() -> IssueAggregate {
    IssueAggregate {
        issue: Issue {
            id: IssueId::new(1),
            project_id: ProjectId::new(1),
            subject: "issue1".to_string(),
            description: String::new(),
            status_id: IssueStatusId::new(3),
        },
        author_id: UserId::new(1001),
        created_on: local_date(2026, 1, 1),
        updated_on: local_date(2026, 1, 4),
        tracker_id: TrackerId::new(1),
        priority_id: PriorityId::new(1),
        assigned_to_id: Some(UserId::new(1001)),
        target_version_id: Some(TargetVersionId::new(1)),
        start_date: Some(local_date(2025, 12, 9)),
        due_date: Some(local_date(2025, 12, 19)),
        done_ratio: 100,
        estimated_hours: None,
        total_spent_hours: None,
        category_id: Some(CategoryId::new(1)),
        parent_id: Some(IssueId::new(3)),
        journals: vec![],
    }
}

pub fn sample_closed_child_issue() -> IssueAggregate {
    IssueAggregate {
        issue: Issue {
            id: IssueId::new(2),
            project_id: ProjectId::new(1),
            subject: "issue2".to_string(),
            description: String::new(),
            status_id: IssueStatusId::new(5),
        },
        author_id: UserId::new(1001),
        created_on: local_date(2026, 2, 1),
        updated_on: local_date(2026, 2, 4),
        tracker_id: TrackerId::new(2),
        priority_id: PriorityId::new(1),
        assigned_to_id: Some(UserId::new(1001)),
        target_version_id: None,
        start_date: Some(local_date(2025, 12, 9)),
        due_date: Some(local_date(2025, 12, 19)),
        done_ratio: 100,
        estimated_hours: None,
        total_spent_hours: None,
        category_id: Some(CategoryId::new(1)),
        parent_id: Some(IssueId::new(3)),
        journals: vec![],
    }
}

/// 長いsubject、Markdown本文、子Issue 1・2を持つ。
pub fn sample_parent_issue() -> IssueAggregate {
    IssueAggregate {
        issue: Issue {
            id: IssueId::new(3),
            project_id: ProjectId::new(1),
            subject: "issue1(長ああああああああああああああああああああああああああああああああああああいタイトル)"
                .to_string(),
            description: SAMPLE_MARKDOWN.to_string(),
            status_id: IssueStatusId::new(3),
        },
        author_id: UserId::new(1001),
        created_on: local_date(2026, 2, 4),
        updated_on: local_date(2026, 2, 16),
        tracker_id: TrackerId::new(3),
        priority_id: PriorityId::new(1),
        assigned_to_id: Some(UserId::new(1001)),
        target_version_id: None,
        start_date: Some(local_date(2026, 2, 16)),
        due_date: Some(local_date(2026, 2, 17)),
        done_ratio: 0,
        estimated_hours: None,
        total_spent_hours: None,
        category_id: Some(CategoryId::new(1)),
        parent_id: None,
        journals: vec![],
    }
}

/// `sample_parent_issue`の詳細取得で得られる子一覧。子Issue 1・2のID・トラッカー・題名を持つ。
pub fn sample_parent_issue_children() -> Vec<IssueChild> {
    vec![
        IssueChild {
            id: IssueId::new(1),
            tracker_id: TrackerId::new(1),
            subject: "issue1".to_string(),
            children: vec![],
        },
        IssueChild {
            id: IssueId::new(2),
            tracker_id: TrackerId::new(2),
            subject: "issue2".to_string(),
            children: vec![],
        },
    ]
}

/// `sample_parent_issue`を、子一覧とともに詳細取得した状態にするAction。
pub fn fetch_sample_parent_issue_actions(journals: Vec<Journal>) -> [Action; 2] {
    let mut issue = sample_parent_issue();
    issue.journals = journals;
    fetch_issue_actions_with_children(issue, sample_parent_issue_children())
}

/// `issue`を、子一覧なしで詳細取得した状態にするAction。
pub fn fetch_issue_actions(issue: IssueAggregate) -> [Action; 2] {
    fetch_issue_actions_with_children(issue, vec![])
}

fn fetch_issue_actions_with_children(
    issue: IssueAggregate,
    children: Vec<IssueChild>,
) -> [Action; 2] {
    let id = issue.issue.id;
    [
        crate::stores::IssueAction::StartFetching { id }.into(),
        Action::IssueFetchSucceeded {
            id,
            issue,
            children,
        },
    ]
}

/// `fetch_issue_actions`をStoreへ順に適用する。
pub fn load_issue(store: &mut Store, issue: IssueAggregate) {
    for action in fetch_issue_actions(issue) {
        store.consume_action(action);
    }
}

/// `fetch_issue_actions`をDispatcherで順に適用する。
pub fn dispatch_loaded_issue(dispatcher: &mut Dispatcher, issue: IssueAggregate) {
    for action in fetch_issue_actions(issue) {
        dispatcher.dispatch(action);
        dispatcher.consume_action();
    }
}

pub fn sample_parent_issue_journals() -> Vec<Journal> {
    let journal = |id: u16, updated_on, details, notes: &str| Journal {
        id: JournalId::new(id),
        issue_id: IssueId::new(3),
        user: "user1".to_string(),
        updated_on: Some(updated_on),
        details,
        notes: notes.to_string(),
    };
    vec![
        journal(
            1,
            local_date(2026, 2, 10),
            vec![JournalDetail::Attr(JournalDetailAttr::StatusId {
                old: IssueStatusId::new(1),
                new: IssueStatusId::new(2),
            })],
            "",
        ),
        journal(
            2,
            local_date(2026, 2, 16),
            vec![JournalDetail::Attr(JournalDetailAttr::DueDate {
                old: Some(local_date(2026, 2, 16)),
                new: Some(local_date(2026, 2, 17)),
            })],
            "",
        ),
        journal(3, local_date(2026, 2, 16), vec![], SAMPLE_MARKDOWN),
    ]
}
