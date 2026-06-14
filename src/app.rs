use std::collections::{HashMap, VecDeque};

use crate::entities::{Issue, IssueStatus, Journal, JournalPropertyChange};
use crate::libs::yaml::{as_u16_array, as_u16_option};
use crate::libs::{
    as_bool, as_local_datetime, as_local_datetime_option, as_string, as_string_array,
    as_string_option, as_u16, read_yaml,
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

pub enum IssueState {
    Synced,
    Updated,
}

pub enum JournalState {
    Synced,
    Updated,
}

#[derive(PartialEq, Eq)]
pub enum IssueStatusState {
    Existing,
    Deleted,
}

pub struct Store {
    issues: HashMap<u16, (Issue, IssueState)>,
    journals: HashMap<u16, (Journal, JournalState)>,
    issue_statuses: HashMap<u16, (IssueStatus, IssueStatusState)>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            issues: HashMap::new(),
            journals: HashMap::new(),
            issue_statuses: HashMap::new(),
        }
    }

    pub fn consume_action(&mut self, action: Action) {
        match action {
            Action::LoadIssueStatuses => {
                if self.issue_statuses.is_empty() {
                    self.issue_statuses = parse_issue_statuses_yaml();
                }
            }
            Action::LoadIssue { id } => {
                if self.issues.get(&id).is_none() {
                    self.issues
                        .insert(id, (parse_issue_yaml(id), IssueState::Synced));
                }
            }
            Action::UpdateIssue { id, body } => {
                if let Some((issue, state)) = self.issues.get_mut(&id) {
                    issue.body = body;
                    *state = IssueState::Updated;
                }
            }
            Action::UpdateIssueStatus { id, status_id } => {
                if let Some((issue, state)) = self.issues.get_mut(&id) {
                    issue.issue_status_id = status_id
                }
            }
            Action::LoadJournal { id } => {
                if self.journals.get(&id).is_none() {
                    self.journals
                        .insert(id, (parse_journal_yaml(id), JournalState::Synced));
                }
            }
            Action::UpdateJournal { id, body } => {
                if let Some((journal, state)) = self.journals.get_mut(&id) {
                    journal.comment = Some(body);
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

    pub fn get_issue_statuses(&self) -> &HashMap<u16, (IssueStatus, IssueStatusState)> {
        &self.issue_statuses
    }

    pub fn get_issue_status(&self, issue_status_id: u16) -> &(IssueStatus, IssueStatusState) {
        self.issue_statuses
            .get(&issue_status_id)
            .expect("issue status must exist")
    }
}

pub enum Action {
    LoadIssueStatuses,
    LoadIssue { id: u16 },
    UpdateIssue { id: u16, body: String },
    UpdateIssueStatus { id: u16, status_id: u16 },
    LoadJournal { id: u16 },
    UpdateJournal { id: u16, body: String },
}

fn parse_journal_yaml(id: u16) -> Journal {
    let path = format!("datas/journals/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let creator = as_string(&yaml, "creator");
    let updated_at = as_local_datetime(&yaml, "updated_at");
    let journal_type = yaml["type"].as_str();
    let properties = if let Some(entries) = yaml["properties"].as_vec() {
        entries
            .iter()
            .map(|entry| JournalPropertyChange {
                target: entry["target"].as_str().expect("no target").to_string(),
                old: entry["old"].as_str().expect("no old").to_string(),
                new: entry["new"].as_str().expect("no new").to_string(),
            })
            .collect()
    } else if journal_type == Some("property") {
        vec![JournalPropertyChange {
            target: as_string(&yaml, "target"),
            old: as_string(&yaml, "old"),
            new: as_string(&yaml, "new"),
        }]
    } else {
        vec![]
    };
    let comment = yaml["body"].as_str().map(|body| body.to_string());

    if properties.is_empty() && comment.is_none() {
        panic!("no matching journal content");
    }

    Journal {
        id,
        creator,
        updated_at,
        properties,
        comment,
    }
}

fn parse_issue_yaml(id: u16) -> Issue {
    let path = format!("datas/issues/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let id = as_u16(&yaml, "id");
    let title = as_string(&yaml, "title");
    let creator = as_string(&yaml, "creator");
    let appended_at = as_local_datetime(&yaml, "appended_at");
    let updated_at = as_local_datetime(&yaml, "updated_at");
    let issue_status_id = as_u16(&yaml, "issue_status_id");
    let priority = as_string(&yaml, "priority");
    let person_in_charge = as_string_option(&yaml, "person_in_charge");
    let target_version = as_string_option(&yaml, "target_version");
    let start_date = as_local_datetime_option(&yaml, "start_date");
    let due = as_local_datetime_option(&yaml, "due");
    let progress = as_u16(&yaml, "progress");
    let planned_hours = as_u16_option(&yaml, "planned_hours");
    let resolve_way = as_string_option(&yaml, "resolve_way");
    let component = as_string(&yaml, "component");
    let tags = as_string_array(&yaml, "tags");
    let body = as_string(&yaml, "body");
    let child_ids = as_u16_array(&yaml, "child_ids");
    let journal_ids = as_u16_array(&yaml, "journal_ids");
    Issue {
        id,
        title,
        creator,
        appended_at,
        updated_at,
        issue_status_id,
        priority,
        person_in_charge,
        target_version,
        start_date,
        due,
        progress,
        planned_hours,
        resolve_way,
        component,
        tags,
        body,
        child_ids,
        journal_ids,
    }
}

fn parse_issue_statuses_yaml() -> HashMap<u16, (IssueStatus, IssueStatusState)> {
    let yaml = read_yaml("datas/issue_statuses.yml");
    let entries = yaml["issue_statuses"].as_vec().expect("no issue_statuses");

    entries
        .iter()
        .map(|entry| {
            let status = IssueStatus {
                id: as_u16(entry, "id"),
                name: as_string(entry, "name"),
                is_closed: as_bool(entry, "is_closed"),
            };
            (status.id, (status, IssueStatusState::Existing))
        })
        .collect()
}
