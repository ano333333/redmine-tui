use std::collections::{HashMap, VecDeque};

use crate::entities::{
    Category, Issue, IssueStatus, Journal, Priority, Project, TargetVersion, TimeEntityActivity,
    Tracker, User,
};
use crate::libs::yaml::{
    parse_categories_yaml, parse_issue_statuses_yaml, parse_issue_yaml, parse_journal_yaml,
    parse_priorities_yaml, parse_projects_yaml, parse_target_versions_yaml,
    parse_time_entity_activities_yaml, parse_trackers_yaml, parse_users_yaml,
};
use crate::vos::issue_property_diff::{
    IssueAssignedToIdDiff, IssueCategoryIdDiff, IssueDescriptionDiff, IssueDoneRatioDiff,
    IssueDueDateDiff, IssueStartDateDiff, IssueStatusIdDiff, IssueTargetVersionIdDiff,
};
use crate::vos::{
    CategoryId, EntityIdValue, IssuePropertyDiff, IssueStatusId, PriorityId, ProjectId,
    TargetVersionId, TimeEntityActivityId, UserId,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueState {
    Synced,
    Updated,
}

pub enum JournalState {
    Synced,
    Updated,
}

pub struct Store {
    issues: HashMap<u16, Issue>,
    issue_property_diffs: HashMap<u16, Vec<IssuePropertyDiff>>,
    journals: HashMap<u16, (Journal, JournalState)>,
    users: HashMap<UserId, User>,
    issue_statuses: HashMap<u16, IssueStatus>,
    priorities: HashMap<u16, Priority>,
    projects: HashMap<u16, Project>,
    trackers: HashMap<u16, Tracker>,
    target_versions: HashMap<TargetVersionId, TargetVersion>,
    categories: HashMap<CategoryId, Category>,
    time_entity_activities: HashMap<TimeEntityActivityId, TimeEntityActivity>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            issues: HashMap::new(),
            issue_property_diffs: HashMap::new(),
            journals: HashMap::new(),
            users: HashMap::new(),
            issue_statuses: HashMap::new(),
            priorities: HashMap::new(),
            projects: HashMap::new(),
            trackers: HashMap::new(),
            target_versions: HashMap::new(),
            categories: HashMap::new(),
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
            Action::LoadTargetVersions => {
                if self.target_versions.is_empty() {
                    self.target_versions = parse_target_versions_yaml();
                }
            }
            Action::LoadCategories => {
                if self.categories.is_empty() {
                    self.categories = parse_categories_yaml();
                }
            }
            Action::LoadTimeEntityActivities => {
                if self.time_entity_activities.is_empty() {
                    self.time_entity_activities = parse_time_entity_activities_yaml();
                }
            }
            Action::LoadIssue { id } => {
                self.issues.entry(id).or_insert(parse_issue_yaml(id));
                self.issue_property_diffs.entry(id).or_default();
            }
            Action::UpdateIssue { id, body } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.description.clone();
                    issue.description = body.clone();
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::Description(IssueDescriptionDiff {
                            before,
                            after: body,
                        }),
                    );
                }
            }
            Action::UpdateIssueStatus { id, status_id } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.status_id;
                    issue.status_id = status_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                            before,
                            after: status_id,
                        }),
                    );
                }
            }
            Action::UpdateIssueAssignedTo { id, assigned_to_id } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.assigned_to_id;
                    issue.assigned_to_id = assigned_to_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::AssignedToId(IssueAssignedToIdDiff {
                            before,
                            after: assigned_to_id,
                        }),
                    );
                }
            }
            Action::UpdateIssueTargetVersion {
                id,
                target_version_id,
            } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.target_version_id;
                    issue.target_version_id = target_version_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::TargetVersionId(IssueTargetVersionIdDiff {
                            before,
                            after: target_version_id,
                        }),
                    );
                }
            }
            Action::UpdateIssueCategory { id, category_id } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.category_id;
                    issue.category_id = category_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::CategoryId(IssueCategoryIdDiff {
                            before,
                            after: category_id,
                        }),
                    );
                }
            }
            Action::UpdateIssueDoneRatio { id, done_ratio } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.done_ratio;
                    issue.done_ratio = done_ratio;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::DoneRatio(IssueDoneRatioDiff {
                            before,
                            after: done_ratio,
                        }),
                    );
                }
            }
            Action::UpdateIssueStartDate { id, start_date } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.start_date;
                    issue.start_date = start_date;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::StartDate(IssueStartDateDiff {
                            before,
                            after: start_date,
                        }),
                    );
                }
            }
            Action::UpdateIssueDueDate { id, due_date } => {
                if let Some(issue) = self.issues.get_mut(&id) {
                    let before = issue.due_date;
                    issue.due_date = due_date;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::DueDate(IssueDueDateDiff {
                            before,
                            after: due_date,
                        }),
                    );
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

    pub fn get_issue(&self, issue_id: u16) -> Option<(&Issue, IssueState)> {
        self.issues
            .get(&issue_id)
            .map(|issue| (issue, self.get_issue_state(issue_id)))
    }

    pub fn get_issue_property_diffs(&self, issue_id: u16) -> &[IssuePropertyDiff] {
        self.issue_property_diffs
            .get(&issue_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn get_issue_state(&self, issue_id: u16) -> IssueState {
        if self.get_issue_property_diffs(issue_id).is_empty() {
            IssueState::Synced
        } else {
            IssueState::Updated
        }
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

    pub fn get_target_versions(&self) -> &HashMap<TargetVersionId, TargetVersion> {
        &self.target_versions
    }

    pub fn get_target_version(&self, target_version_id: TargetVersionId) -> Option<&TargetVersion> {
        self.target_versions.get(&target_version_id)
    }

    pub fn get_categories(&self) -> &HashMap<CategoryId, Category> {
        &self.categories
    }

    pub fn get_category(&self, category_id: CategoryId) -> Option<&Category> {
        self.categories.get(&category_id)
    }

    pub fn get_time_entity_activities(&self) -> &HashMap<TimeEntityActivityId, TimeEntityActivity> {
        &self.time_entity_activities
    }

    pub fn projects(&self) -> &HashMap<u16, Project> {
        &self.projects
    }
}

pub enum Action {
    LoadUsers,
    LoadIssueStatuses,
    LoadPriorities,
    LoadProjects,
    LoadTrackers,
    LoadTargetVersions,
    LoadCategories,
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
    UpdateIssueTargetVersion {
        id: u16,
        target_version_id: Option<TargetVersionId>,
    },
    UpdateIssueCategory {
        id: u16,
        category_id: Option<CategoryId>,
    },
    UpdateIssueDoneRatio {
        id: u16,
        done_ratio: u16,
    },
    UpdateIssueStartDate {
        id: u16,
        start_date: Option<chrono::DateTime<chrono::Local>>,
    },
    UpdateIssueDueDate {
        id: u16,
        due_date: Option<chrono::DateTime<chrono::Local>>,
    },
    LoadJournal {
        id: u16,
    },
    UpdateJournal {
        id: u16,
        notes: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::local_datetime;
    use crate::vos::IssuePropertyDiff;
    use crate::vos::issue_property_diff::{IssueDueDateDiff, IssueStartDateDiff};
    use crate::vos::{CategoryId, TargetVersionId};

    #[test]
    fn load_target_versions_populates_store() {
        let mut store = Store::new();

        store.consume_action(Action::LoadTargetVersions);

        let target_version = store
            .get_target_version(TargetVersionId::new(1))
            .expect("target version should be loaded");
        assert_eq!(target_version.name, "v1.2.3");
        assert_eq!(store.get_target_versions().len(), 1);
    }

    #[test]
    fn load_issue_reads_target_version_id_reference() {
        let mut store = Store::new();

        store.consume_action(Action::LoadIssue { id: 1 });

        let (issue, _) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.target_version_id, Some(TargetVersionId::new(1)));
    }

    #[test]
    fn update_issue_target_version_sets_selected_version() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 2 });

        store.consume_action(Action::UpdateIssueTargetVersion {
            id: 2,
            target_version_id: Some(TargetVersionId::new(1)),
        });

        let (issue, state) = store.get_issue(2).expect("issue should be loaded");
        assert_eq!(issue.target_version_id, Some(TargetVersionId::new(1)));
        assert_eq!(state, IssueState::Updated);
    }

    #[test]
    fn update_issue_target_version_can_clear_version() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1 });

        store.consume_action(Action::UpdateIssueTargetVersion {
            id: 1,
            target_version_id: None,
        });

        let (issue, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.target_version_id, None);
        assert_eq!(state, IssueState::Updated);
    }

    #[test]
    fn load_categories_populates_store() {
        let mut store = Store::new();

        store.consume_action(Action::LoadCategories);

        let category = store
            .get_category(CategoryId::new(1))
            .expect("category should be loaded");
        assert_eq!(category.name, "category1");
        assert_eq!(store.get_categories().len(), 1);
    }

    #[test]
    fn load_issue_reads_category_id_reference() {
        let mut store = Store::new();

        store.consume_action(Action::LoadIssue { id: 1 });

        let (issue, _) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.category_id, Some(CategoryId::new(1)));
    }

    #[test]
    fn update_issue_category_sets_selected_category() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1 });

        store.consume_action(Action::UpdateIssueCategory {
            id: 1,
            category_id: Some(CategoryId::new(2)),
        });

        let (issue, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.category_id, Some(CategoryId::new(2)));
        assert_eq!(state, IssueState::Updated);
    }

    #[test]
    fn update_issue_category_can_clear_category() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1 });

        store.consume_action(Action::UpdateIssueCategory {
            id: 1,
            category_id: None,
        });

        let (issue, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.category_id, None);
        assert_eq!(state, IssueState::Updated);
    }

    #[test]
    fn update_issue_start_date_updates_issue_and_records_diff() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1 });
        let before = store.get_issue(1).unwrap().0.start_date;
        let after = Some(local_datetime("2026-04-30T00:00:00+09:00"));

        store.consume_action(Action::UpdateIssueStartDate {
            id: 1,
            start_date: after,
        });

        assert_eq!(store.get_issue(1).unwrap().0.start_date, after);
        assert_eq!(
            store.get_issue_property_diffs(1).last(),
            Some(&IssuePropertyDiff::StartDate(IssueStartDateDiff {
                before,
                after
            }))
        );
    }

    #[test]
    fn update_issue_due_date_updates_issue_and_records_diff() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1 });
        let before = store.get_issue(1).unwrap().0.due_date;
        let after = Some(local_datetime("2026-05-01T00:00:00+09:00"));

        store.consume_action(Action::UpdateIssueDueDate {
            id: 1,
            due_date: after,
        });

        assert_eq!(store.get_issue(1).unwrap().0.due_date, after);
        assert_eq!(
            store.get_issue_property_diffs(1).last(),
            Some(&IssuePropertyDiff::DueDate(IssueDueDateDiff {
                before,
                after
            }))
        );
    }
}
