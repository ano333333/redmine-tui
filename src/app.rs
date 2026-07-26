use std::collections::{HashMap, VecDeque};

use crate::entities::{
    Issue, IssueStatus, Journal, Priority, Project, TimeEntityActivity, Tracker, User,
};
use crate::libs::yaml::{as_u16_array, as_u16_option};
use crate::libs::{
    as_bool, as_f64_option, as_local_datetime, as_local_datetime_option, as_string,
    as_string_option, as_u16, read_yaml,
};
use crate::vos::{
    EntityIdValue, IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, PriorityId, ProjectId,
    TimeEntityActivityId, TrackerId, UserId,
};

pub struct Dispatcher {
    store: Store,
    actions: VecDeque<Action>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher {
            store: Store::new(),
            actions: VecDeque::new(),
        }
    }
    pub fn store(&self) -> &Store {
        &self.store
    }
    pub fn dispatch(&mut self, action: Action) {
        self.actions.push_back(action);
    }
    pub fn consume_actinos_len(&self) -> usize {
        self.actions.len()
    }
    pub fn consume_action(&mut self) {
        if let Some(action) = self.actions.pop_front() {
            self.store.consume_action(action);
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum IssueState {
    Synced,
    Updated,
}

pub enum JournalState {
    Synced,
    Updated,
}

pub struct Store {
    issues: HashMap<u16, (Issue, IssueState)>,
    journals: HashMap<u16, (Journal, JournalState)>,
    users: HashMap<UserId, User>,
    issue_statuses: HashMap<u16, IssueStatus>,
    priorities: HashMap<u16, Priority>,
    projects: HashMap<u16, Project>,
    trackers: HashMap<u16, Tracker>,
    time_entity_activities: HashMap<TimeEntityActivityId, TimeEntityActivity>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            issues: HashMap::new(),
            journals: HashMap::new(),
            users: HashMap::new(),
            issue_statuses: HashMap::new(),
            priorities: HashMap::new(),
            projects: HashMap::new(),
            trackers: HashMap::new(),
            time_entity_activities: HashMap::new(),
        }
    }

    pub fn consume_action(&mut self, action: Action) {
        match action {
            Action::LoadUsers => {
                if self.users.is_empty() {
                    self.users = parse_users_yaml();
                }
            }
            Action::LoadIssueStatuses => {
                if self.issue_statuses.is_empty() {
                    self.issue_statuses = parse_issue_statuses_yaml();
                }
            }
            Action::LoadPriorities => {
                if self.priorities.is_empty() {
                    self.priorities = parse_priorities_yaml();
                }
            }
            Action::LoadProjects => {
                if self.projects.is_empty() {
                    self.projects = parse_projects_yaml();
                }
            }
            Action::LoadTrackers => {
                if self.trackers.is_empty() {
                    self.trackers = parse_trackers_yaml();
                }
            }
            Action::LoadTimeEntityActivities => {
                if self.time_entity_activities.is_empty() {
                    self.time_entity_activities = parse_time_entity_activities_yaml();
                }
            }
            Action::LoadIssue { id } => {
                self.issues
                    .entry(id)
                    .or_insert((parse_issue_yaml(id), IssueState::Synced));
            }
            Action::UpdateIssue { id, body } => {
                if let Some((issue, state)) = self.issues.get_mut(&id) {
                    issue.description = body;
                    *state = IssueState::Updated;
                }
            }
            Action::UpdateIssueStatus { id, status_id } => {
                if let Some((issue, _state)) = self.issues.get_mut(&id) {
                    issue.status_id = status_id
                }
            }
            Action::UpdateIssueAssignedTo { id, assigned_to_id } => {
                if let Some((issue, state)) = self.issues.get_mut(&id) {
                    issue.assigned_to_id = assigned_to_id;
                    *state = IssueState::Updated;
                }
            }
            Action::UpdateIssueDoneRatio { id, done_ratio } => {
                if let Some((issue, state)) = self.issues.get_mut(&id) {
                    issue.done_ratio = done_ratio;
                    *state = IssueState::Updated;
                }
            }
            Action::LoadJournal { id } => {
                self.journals
                    .entry(id)
                    .or_insert((parse_journal_yaml(id), JournalState::Synced));
            }
            Action::UpdateJournal { id, notes } => {
                if let Some((journal, state)) = self.journals.get_mut(&id) {
                    journal.notes = notes;
                    *state = JournalState::Updated;
                }
            }
        }
    }

    pub fn get_issue(&self, issue_id: u16) -> Option<&(Issue, IssueState)> {
        self.issues.get(&issue_id)
    }

    pub fn get_journal(&self, journal_id: u16) -> Option<&(Journal, JournalState)> {
        self.journals.get(&journal_id)
    }

    pub fn get_users(&self) -> &HashMap<UserId, User> {
        &self.users
    }

    pub fn get_user(&self, user_id: UserId) -> Option<&User> {
        self.users.get(&user_id)
    }

    pub fn get_issue_statuses(&self) -> &HashMap<u16, IssueStatus> {
        &self.issue_statuses
    }

    pub fn get_issue_status(&self, issue_status_id: IssueStatusId) -> &IssueStatus {
        self.issue_statuses
            .get(&issue_status_id.get())
            .expect("issue status must exist")
    }

    pub fn get_priorities(&self) -> &HashMap<u16, Priority> {
        &self.priorities
    }

    pub fn get_priority(&self, priority_id: PriorityId) -> Option<&Priority> {
        self.priorities.get(&priority_id.get())
    }

    pub fn get_projects(&self) -> &HashMap<u16, Project> {
        &self.projects
    }

    pub fn get_project(&self, project_id: ProjectId) -> Option<&Project> {
        self.projects.get(&project_id.get())
    }

    pub fn get_trackers(&self) -> &HashMap<u16, Tracker> {
        &self.trackers
    }

    pub fn get_tracker(&self, tracker_id: u16) -> Option<&Tracker> {
        self.trackers.get(&tracker_id)
    }

    pub fn get_time_entity_activities(&self) -> &HashMap<TimeEntityActivityId, TimeEntityActivity> {
        &self.time_entity_activities
    }
}

pub enum Action {
    LoadUsers,
    LoadIssueStatuses,
    LoadPriorities,
    LoadProjects,
    LoadTrackers,
    LoadTimeEntityActivities,
    LoadIssue {
        id: u16,
    },
    UpdateIssue {
        id: u16,
        body: String,
    },
    UpdateIssueStatus {
        id: u16,
        status_id: IssueStatusId,
    },
    UpdateIssueAssignedTo {
        id: u16,
        assigned_to_id: Option<UserId>,
    },
    UpdateIssueDoneRatio {
        id: u16,
        done_ratio: u16,
    },
    LoadJournal {
        id: u16,
    },
    UpdateJournal {
        id: u16,
        notes: String,
    },
}

fn parse_journal_yaml(id: u16) -> Journal {
    let path = format!("datas/journals/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let parsed_id = as_u16(&yaml, "id");
    assert_eq!(
        parsed_id, id,
        "journal id mismatch: {} != {}",
        parsed_id, id
    );
    let user = as_string(&yaml, "user");
    let updated_on = as_local_datetime(&yaml, "updated_on");
    let notes = read_required_string_option(&yaml, "notes").unwrap_or_default();
    let details = yaml["details"]
        .as_vec()
        .expect("no details")
        .iter()
        .map(parse_journal_detail_yaml)
        .collect();

    Journal {
        id: id.into(),
        user,
        updated_on,
        details,
        notes,
    }
}

fn parse_journal_detail_yaml(yaml: &yaml_rust::Yaml) -> JournalDetail {
    match as_string(yaml, "type").as_str() {
        "attr" => JournalDetail::Attr(parse_journal_detail_attr_yaml(yaml)),
        detail_type => panic!("unsupported journal detail type: {}", detail_type),
    }
}

fn parse_journal_detail_attr_yaml(yaml: &yaml_rust::Yaml) -> JournalDetailAttr {
    match as_string(yaml, "name").as_str() {
        "status_id" => JournalDetailAttr::StatusId {
            old: as_string(yaml, "old"),
            new: as_string(yaml, "new"),
        },
        "due_date" => JournalDetailAttr::DueDate {
            old: as_local_datetime(yaml, "old"),
            new: as_local_datetime(yaml, "new"),
        },
        "assigned_to" => JournalDetailAttr::AssignedTo {
            old: read_required_string_option(yaml, "old"),
            new: read_required_string_option(yaml, "new"),
        },
        attr_name => panic!("unsupported journal detail attr: {}", attr_name),
    }
}

fn read_required_string_option(yaml: &yaml_rust::Yaml, key: &str) -> Option<String> {
    if yaml[key].is_badvalue() {
        panic!("no {}", key);
    }
    as_string_option(yaml, key)
}

fn parse_issue_yaml(id: u16) -> Issue {
    let path = format!("datas/issues/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let id = IssueId::new(as_u16(&yaml, "id"));
    let subject = as_string(&yaml, "subject");
    let author_id = UserId::new(as_u16(&yaml, "author_id"));
    let created_on = as_local_datetime(&yaml, "created_on");
    let updated_on = as_local_datetime(&yaml, "updated_on");
    let project_id = ProjectId::new(as_u16(&yaml, "project_id"));
    let tracker_id = TrackerId::new(as_u16(&yaml, "tracker_id"));
    let status_id = IssueStatusId::new(as_u16(&yaml, "status_id"));
    let priority_id = PriorityId::new(as_u16(&yaml, "priority_id"));
    let assigned_to_id = as_u16_option(&yaml, "assigned_to_id").map(UserId::new);
    let fixed_version = as_string_option(&yaml, "fixed_version");
    let start_date = as_local_datetime_option(&yaml, "start_date");
    let due_date = as_local_datetime_option(&yaml, "due_date");
    let done_ratio = as_u16(&yaml, "done_ratio");
    let estimated_hours = as_u16_option(&yaml, "estimated_hours");
    let total_spent_hours = as_f64_option(&yaml, "total_spent_hours");
    let resolve_way = as_string_option(&yaml, "resolve_way");
    let component = as_string(&yaml, "component");
    let description = as_string(&yaml, "description");
    let child_ids = as_u16_array(&yaml, "child_ids")
        .into_iter()
        .map(IssueId::new)
        .collect();
    let journal_ids = as_u16_array(&yaml, "journal_ids");
    Issue {
        id,
        subject,
        author_id,
        created_on,
        updated_on,
        project_id,
        tracker_id,
        status_id,
        priority_id,
        assigned_to_id,
        fixed_version,
        start_date,
        due_date,
        done_ratio,
        estimated_hours,
        total_spent_hours,
        resolve_way,
        component,
        description,
        child_ids,
        journal_ids,
    }
}

fn parse_users_yaml() -> HashMap<UserId, User> {
    let yaml = read_yaml("datas/users.yml");
    let entries = yaml["users"].as_vec().expect("no users");

    entries
        .iter()
        .map(|entry| {
            let user = User {
                id: UserId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (user.id, user)
        })
        .collect()
}

fn parse_issue_statuses_yaml() -> HashMap<u16, IssueStatus> {
    let yaml = read_yaml("datas/issue_statuses.yml");
    let entries = yaml["issue_statuses"].as_vec().expect("no issue_statuses");

    entries
        .iter()
        .map(|entry| {
            let status = IssueStatus {
                id: IssueStatusId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
                is_closed: as_bool(entry, "is_closed"),
            };
            (status.id.get(), status)
        })
        .collect()
}

fn parse_priorities_yaml() -> HashMap<u16, Priority> {
    let yaml = read_yaml("datas/priorities.yml");
    let entries = yaml["priorities"].as_vec().expect("no priorities");

    entries
        .iter()
        .map(|entry| {
            let priority = Priority {
                id: PriorityId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (priority.id.get(), priority)
        })
        .collect()
}

fn parse_projects_yaml() -> HashMap<u16, Project> {
    let yaml = read_yaml("datas/projects.yml");
    let entries = yaml["projects"].as_vec().expect("no projects");

    entries
        .iter()
        .map(|entry| {
            let project = Project {
                id: ProjectId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (project.id.get(), project)
        })
        .collect()
}

fn parse_trackers_yaml() -> HashMap<u16, Tracker> {
    let yaml = read_yaml("datas/trackers.yml");
    let entries = yaml["trackers"].as_vec().expect("no trackers");

    entries
        .iter()
        .map(|entry| {
            let tracker = Tracker {
                id: TrackerId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (tracker.id.get(), tracker)
        })
        .collect()
}

fn parse_time_entity_activities_yaml() -> HashMap<TimeEntityActivityId, TimeEntityActivity> {
    let yaml = read_yaml("datas/time_entity_activities.yml");
    let entries = yaml["time_entity_activities"]
        .as_vec()
        .expect("no time_entity_activities");

    entries
        .iter()
        .map(|entry| {
            let act = TimeEntityActivity {
                id: TimeEntityActivityId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
                is_default: as_bool(entry, "is_default"),
            };
            (act.id, act)
        })
        .collect()
}
