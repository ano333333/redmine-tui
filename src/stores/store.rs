use std::collections::{HashMap, VecDeque};

use crate::entities::{
    Category, Issue, IssueStatus, Journal, Priority, Project, TargetVersion, TimeEntityActivity,
    Tracker, User,
};
use crate::libs::yaml::{parse_issue_yaml, parse_journal_yaml};
use crate::vos::issue_property_diff::{
    IssueAssignedToIdDiff, IssueCategoryIdDiff, IssueDescriptionDiff, IssueDoneRatioDiff,
    IssueDueDateDiff, IssueStartDateDiff, IssueStatusIdDiff, IssueTargetVersionIdDiff,
};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssuePropertyDiff, IssueStatusId, JournalId, PriorityId,
    ProjectId, TargetVersionId, TimeEntityActivityId, TrackerId, UserId,
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
    Edited,
    Uploading,
}

pub enum JournalState {
    Synced,
    Updated,
}

pub struct Store {
    issues: HashMap<IssueId, Issue>,
    issue_states: HashMap<IssueId, IssueState>,
    issue_property_diffs: HashMap<IssueId, Vec<IssuePropertyDiff>>,
    issue_upload_conflicts: HashMap<IssueId, (Issue, Vec<IssuePropertyDiff>)>,
    journals: HashMap<JournalId, (Journal, JournalState)>,
    users: HashMap<UserId, User>,
    issue_statuses: HashMap<IssueStatusId, IssueStatus>,
    priorities: HashMap<PriorityId, Priority>,
    projects: HashMap<ProjectId, Project>,
    trackers: HashMap<TrackerId, Tracker>,
    target_versions: HashMap<TargetVersionId, TargetVersion>,
    categories: HashMap<CategoryId, Category>,
    time_entity_activities: HashMap<TimeEntityActivityId, TimeEntityActivity>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            issues: HashMap::new(),
            issue_states: HashMap::new(),
            issue_property_diffs: HashMap::new(),
            issue_upload_conflicts: HashMap::new(),
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
            Action::SyncUsers { users } => {
                self.users = users.into_iter().map(|user| (user.id, user)).collect();
            }
            Action::SyncIssueStatuses { issue_statuses } => {
                self.issue_statuses = issue_statuses
                    .into_iter()
                    .map(|issue_status| (issue_status.id, issue_status))
                    .collect();
            }
            Action::SyncPriorities { priorities } => {
                self.priorities = priorities
                    .into_iter()
                    .map(|priority| (priority.id, priority))
                    .collect();
            }
            Action::SyncProjects { projects } => {
                self.projects = projects
                    .into_iter()
                    .map(|project| (project.id, project))
                    .collect();
            }
            Action::SyncTrackers { trackers } => {
                self.trackers = trackers
                    .into_iter()
                    .map(|tracker| (tracker.id, tracker))
                    .collect();
            }
            Action::SyncTargetVersions { target_versions } => {
                self.target_versions = target_versions
                    .into_iter()
                    .map(|target_version| (target_version.id, target_version))
                    .collect();
            }
            Action::SyncCategories { categories } => {
                self.categories = categories
                    .into_iter()
                    .map(|category| (category.id, category))
                    .collect();
            }
            Action::SyncTimeEntityActivities {
                time_entity_activities,
            } => {
                self.time_entity_activities = time_entity_activities
                    .into_iter()
                    .map(|activity| (activity.id, activity))
                    .collect();
            }
            Action::LoadIssue { id } => {
                self.issues.entry(id).or_insert(parse_issue_yaml(id.get()));
                self.issue_states.entry(id).or_insert(IssueState::Synced);
                self.issue_property_diffs.entry(id).or_default();
            }
            Action::SyncIssue { issue } => {
                let id = issue.id;
                if self.issues.contains_key(&id) {
                    let state = self.get_issue_state(id);
                    if state == IssueState::Synced {
                        panic!("cannot sync issue {id} while it is {state:?}");
                    }
                }
                self.issues.insert(id, issue);
                self.issue_property_diffs.remove(&id);
                self.issue_upload_conflicts.remove(&id);
                self.issue_states.insert(id, IssueState::Synced);
            }
            Action::StartIssueUpload { id } => {
                let state = self.get_issue_state(id);
                if state != IssueState::Edited {
                    panic!("cannot start issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
                self.issue_states.insert(id, IssueState::Uploading);
            }
            Action::CancelIssueUpload { id } => {
                let state = self.get_issue_state(id);
                if state != IssueState::Uploading {
                    panic!("cannot cancel issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
                self.issue_states.insert(id, IssueState::Edited);
            }
            Action::ClearIssueUploadConflicts { id } => {
                let state = self.get_issue_state(id);
                if state != IssueState::Uploading {
                    panic!("cannot clear issue upload conflicts while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
            }
            Action::FailIssueUpload { id } => {
                let state = self.get_issue_state(id);
                if state != IssueState::Uploading {
                    panic!("cannot fail issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
                self.issue_states.insert(id, IssueState::Edited);
            }
            Action::IssueUploadConflictsDetected {
                server_issue,
                conflicts,
            } => {
                let id = server_issue.id;
                let state = self.get_issue_state(id);
                if state != IssueState::Uploading {
                    panic!("cannot retain issue upload conflicts while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts
                    .insert(id, (server_issue, conflicts));
            }
            Action::UpdateIssue { id, body } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.description.clone();
                    issue.description = body.clone();
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::Description(IssueDescriptionDiff {
                            before,
                            after: body,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueStatus { id, status_id } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.status_id;
                    issue.status_id = status_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                            before,
                            after: status_id,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueAssignedTo { id, assigned_to_id } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.assigned_to_id;
                    issue.assigned_to_id = assigned_to_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::AssignedToId(IssueAssignedToIdDiff {
                            before,
                            after: assigned_to_id,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueTargetVersion {
                id,
                target_version_id,
            } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.target_version_id;
                    issue.target_version_id = target_version_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::TargetVersionId(IssueTargetVersionIdDiff {
                            before,
                            after: target_version_id,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueCategory { id, category_id } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.category_id;
                    issue.category_id = category_id;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::CategoryId(IssueCategoryIdDiff {
                            before,
                            after: category_id,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueDoneRatio { id, done_ratio } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.done_ratio;
                    issue.done_ratio = done_ratio;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::DoneRatio(IssueDoneRatioDiff {
                            before,
                            after: done_ratio,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueStartDate { id, start_date } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.start_date;
                    issue.start_date = start_date;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::StartDate(IssueStartDateDiff {
                            before,
                            after: start_date,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::UpdateIssueDueDate { id, due_date } => {
                if self.can_update_issue(id)
                    && let Some(issue) = self.issues.get_mut(&id)
                {
                    let before = issue.due_date;
                    issue.due_date = due_date;
                    self.issue_property_diffs.entry(id).or_default().push(
                        IssuePropertyDiff::DueDate(IssueDueDateDiff {
                            before,
                            after: due_date,
                        }),
                    );
                    self.issue_states.insert(id, IssueState::Edited);
                }
            }
            Action::LoadJournal { id } => {
                self.journals
                    .entry(id)
                    .or_insert((parse_journal_yaml(id), JournalState::Synced));
            }
            Action::UpdateJournal { id, notes } => {
                if let Some((journal, state)) = self.journals.get_mut(&id.into()) {
                    journal.notes = notes;
                    *state = JournalState::Updated;
                }
            }
        }
    }

    pub fn get_issue(&self, issue_id: impl Into<IssueId>) -> Option<(&Issue, IssueState)> {
        let issue_id = issue_id.into();
        self.issues
            .get(&issue_id)
            .map(|issue| (issue, self.get_issue_state(issue_id)))
    }

    pub fn get_issues(&self) -> &HashMap<IssueId, Issue> {
        &self.issues
    }

    pub fn get_issue_property_diffs(&self, issue_id: impl Into<IssueId>) -> &[IssuePropertyDiff] {
        self.issue_property_diffs
            .get(&issue_id.into())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn get_issue_state(&self, issue_id: impl Into<IssueId>) -> IssueState {
        self.issue_states
            .get(&issue_id.into())
            .copied()
            .unwrap_or(IssueState::Synced)
    }

    pub fn get_journal(
        &self,
        journal_id: impl Into<JournalId>,
    ) -> Option<&(Journal, JournalState)> {
        self.journals.get(&journal_id.into())
    }

    pub fn get_users(&self) -> &HashMap<UserId, User> {
        &self.users
    }

    pub fn get_user(&self, user_id: UserId) -> Option<&User> {
        self.users.get(&user_id)
    }

    pub fn get_issue_statuses(&self) -> &HashMap<IssueStatusId, IssueStatus> {
        &self.issue_statuses
    }

    pub fn get_issue_status(&self, issue_status_id: IssueStatusId) -> &IssueStatus {
        self.issue_statuses
            .get(&issue_status_id)
            .expect("issue status must exist")
    }

    pub fn get_priorities(&self) -> &HashMap<PriorityId, Priority> {
        &self.priorities
    }

    pub fn get_priority(&self, priority_id: impl Into<PriorityId>) -> Option<&Priority> {
        self.priorities.get(&priority_id.into())
    }

    pub fn get_projects(&self) -> &HashMap<ProjectId, Project> {
        &self.projects
    }

    pub fn get_project(&self, project_id: impl Into<ProjectId>) -> Option<&Project> {
        self.projects.get(&project_id.into())
    }

    pub fn get_trackers(&self) -> &HashMap<TrackerId, Tracker> {
        &self.trackers
    }

    pub fn get_tracker(&self, tracker_id: impl Into<TrackerId>) -> Option<&Tracker> {
        self.trackers.get(&tracker_id.into())
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

    pub fn projects(&self) -> &HashMap<ProjectId, Project> {
        &self.projects
    }

    /// 保存処理で検出した競合について、比較時点のサーバー Issue と差分を返す。
    pub fn get_issue_upload_conflict(&self, id: IssueId) -> Option<(&Issue, &[IssuePropertyDiff])> {
        self.issue_upload_conflicts
            .get(&id)
            .map(|(issue, conflicts)| (issue, conflicts.as_slice()))
    }

    fn can_update_issue(&self, id: impl Into<IssueId>) -> bool {
        let id = id.into();
        let state = self.get_issue_state(id);
        if state == IssueState::Uploading {
            panic!("cannot update issue while issue {id} is {state:?}");
        }
        true
    }
}

pub enum Action {
    SyncUsers {
        users: Vec<User>,
    },
    SyncIssueStatuses {
        issue_statuses: Vec<IssueStatus>,
    },
    SyncPriorities {
        priorities: Vec<Priority>,
    },
    SyncProjects {
        projects: Vec<Project>,
    },
    SyncTrackers {
        trackers: Vec<Tracker>,
    },
    SyncTargetVersions {
        target_versions: Vec<TargetVersion>,
    },
    SyncCategories {
        categories: Vec<Category>,
    },
    SyncTimeEntityActivities {
        time_entity_activities: Vec<TimeEntityActivity>,
    },
    LoadIssue {
        id: IssueId,
    },
    SyncIssue {
        issue: Issue,
    },
    StartIssueUpload {
        id: IssueId,
    },
    CancelIssueUpload {
        id: IssueId,
    },
    ClearIssueUploadConflicts {
        id: IssueId,
    },
    FailIssueUpload {
        id: IssueId,
    },
    IssueUploadConflictsDetected {
        server_issue: Issue,
        conflicts: Vec<IssuePropertyDiff>,
    },
    UpdateIssue {
        id: IssueId,
        body: String,
    },
    UpdateIssueStatus {
        id: IssueId,
        status_id: IssueStatusId,
    },
    UpdateIssueAssignedTo {
        id: IssueId,
        assigned_to_id: Option<UserId>,
    },
    UpdateIssueTargetVersion {
        id: IssueId,
        target_version_id: Option<TargetVersionId>,
    },
    UpdateIssueCategory {
        id: IssueId,
        category_id: Option<CategoryId>,
    },
    UpdateIssueDoneRatio {
        id: IssueId,
        done_ratio: u16,
    },
    UpdateIssueStartDate {
        id: IssueId,
        start_date: Option<chrono::DateTime<chrono::Local>>,
    },
    UpdateIssueDueDate {
        id: IssueId,
        due_date: Option<chrono::DateTime<chrono::Local>>,
    },
    LoadJournal {
        id: JournalId,
    },
    UpdateJournal {
        id: JournalId,
        notes: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{local_datetime, sample_issue, sync_fixture_entities};
    use crate::vos::IssuePropertyDiff;
    use crate::vos::issue_property_diff::{IssueDueDateDiff, IssueStartDateDiff};
    use crate::vos::{
        CategoryId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TimeEntityActivityId,
        TrackerId, UserId,
    };

    #[test]
    fn sync_fixture_entities_populates_target_versions() {
        let mut store = Store::new();

        sync_fixture_entities(&mut store);

        let target_version = store
            .get_target_version(TargetVersionId::new(1))
            .expect("target version should be loaded");
        assert_eq!(target_version.name, "v1.2.3");
        assert_eq!(store.get_target_versions().len(), 1);
    }

    #[test]
    fn load_issue_reads_target_version_id_reference() {
        let mut store = Store::new();

        store.consume_action(Action::LoadIssue { id: 1.into() });

        let (issue, _) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.target_version_id, Some(TargetVersionId::new(1)));
    }

    #[test]
    fn update_issue_target_version_sets_selected_version() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 2.into() });

        store.consume_action(Action::UpdateIssueTargetVersion {
            id: 2.into(),
            target_version_id: Some(TargetVersionId::new(1)),
        });

        let (issue, state) = store.get_issue(2).expect("issue should be loaded");
        assert_eq!(issue.target_version_id, Some(TargetVersionId::new(1)));
        assert_eq!(state, IssueState::Edited);
    }

    #[test]
    fn update_issue_target_version_can_clear_version() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::UpdateIssueTargetVersion {
            id: 1.into(),
            target_version_id: None,
        });

        let (issue, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.target_version_id, None);
        assert_eq!(state, IssueState::Edited);
    }

    #[test]
    fn sync_fixture_entities_populates_categories() {
        let mut store = Store::new();

        sync_fixture_entities(&mut store);

        let category = store
            .get_category(CategoryId::new(1))
            .expect("category should be loaded");
        assert_eq!(category.name, "category1");
        assert_eq!(store.get_categories().len(), 1);
    }

    #[test]
    fn load_issue_reads_category_id_reference() {
        let mut store = Store::new();

        store.consume_action(Action::LoadIssue { id: 1.into() });

        let (issue, _) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.category_id, Some(CategoryId::new(1)));
    }

    #[test]
    fn update_issue_category_sets_selected_category() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::UpdateIssueCategory {
            id: 1.into(),
            category_id: Some(CategoryId::new(2)),
        });

        let (issue, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.category_id, Some(CategoryId::new(2)));
        assert_eq!(state, IssueState::Edited);
    }

    #[test]
    fn update_issue_category_can_clear_category() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::UpdateIssueCategory {
            id: 1.into(),
            category_id: None,
        });

        let (issue, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(issue.category_id, None);
        assert_eq!(state, IssueState::Edited);
    }

    #[test]
    fn update_issue_start_date_updates_issue_and_records_diff() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        let before = store.get_issue(1).unwrap().0.start_date;
        let after = Some(local_datetime("2026-04-30T00:00:00+09:00"));

        store.consume_action(Action::UpdateIssueStartDate {
            id: 1.into(),
            start_date: after,
        });

        assert_eq!(store.get_issue(1).unwrap().0.start_date, after);
        assert_eq!(
            store.get_issue_property_diffs(IssueId::new(1)).last(),
            Some(&IssuePropertyDiff::StartDate(IssueStartDateDiff {
                before,
                after
            }))
        );
    }

    #[test]
    fn update_issue_due_date_updates_issue_and_records_diff() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        let before = store.get_issue(1).unwrap().0.due_date;
        let after = Some(local_datetime("2026-05-01T00:00:00+09:00"));

        store.consume_action(Action::UpdateIssueDueDate {
            id: 1.into(),
            due_date: after,
        });

        assert_eq!(store.get_issue(1).unwrap().0.due_date, after);
        assert_eq!(
            store.get_issue_property_diffs(IssueId::new(1)).last(),
            Some(&IssuePropertyDiff::DueDate(IssueDueDateDiff {
                before,
                after
            }))
        );
    }

    #[test]
    fn start_issue_upload_marks_issue_uploading_and_retains_diffs() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });

        store.consume_action(Action::StartIssueUpload { id: 1.into() });

        let (_, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(state, IssueState::Uploading);
        assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
    }

    #[test]
    fn issue_upload_conflicts_are_retained_while_uploading() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "local body".to_string(),
        });
        store.consume_action(Action::StartIssueUpload { id: 1.into() });
        let server_issue = sample_issue(1, "server issue", 1.into(), None, None, None, 0);
        let conflicts = store.get_issue_property_diffs(IssueId::new(1)).to_vec();

        store.consume_action(Action::IssueUploadConflictsDetected {
            server_issue: server_issue.clone(),
            conflicts: conflicts.clone(),
        });

        let (actual_issue, actual_conflicts) = store
            .get_issue_upload_conflict(1.into())
            .expect("issue upload conflict should be retained");
        assert_eq!(actual_issue.subject, server_issue.subject);
        assert_eq!(actual_conflicts, conflicts);
        assert_eq!(store.get_issue(1).unwrap().1, IssueState::Uploading);
    }

    #[test]
    #[should_panic(expected = "cannot update issue while issue 1 is Uploading")]
    fn uploading_issue_update_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });
        store.consume_action(Action::StartIssueUpload { id: 1.into() });

        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "late edit".to_string(),
        });
    }

    #[test]
    #[should_panic(expected = "cannot start issue upload while issue 1 is Uploading")]
    fn uploading_issue_start_upload_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });
        store.consume_action(Action::StartIssueUpload { id: 1.into() });

        store.consume_action(Action::StartIssueUpload { id: 1.into() });
    }

    #[test]
    #[should_panic(expected = "cannot start issue upload while issue 1 is Synced")]
    fn synced_issue_start_upload_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::StartIssueUpload { id: 1.into() });
    }

    #[test]
    #[should_panic(expected = "cannot cancel issue upload while issue 1 is Synced")]
    fn synced_issue_cancel_upload_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::CancelIssueUpload { id: 1.into() });
    }

    #[test]
    #[should_panic(expected = "cannot cancel issue upload while issue 1 is Edited")]
    fn edited_issue_cancel_upload_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });

        store.consume_action(Action::CancelIssueUpload { id: 1.into() });
    }

    #[test]
    #[should_panic(expected = "cannot fail issue upload while issue 1 is Synced")]
    fn synced_issue_fail_upload_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::FailIssueUpload { id: 1.into() });
    }

    #[test]
    #[should_panic(expected = "cannot fail issue upload while issue 1 is Edited")]
    fn edited_issue_fail_upload_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });

        store.consume_action(Action::FailIssueUpload { id: 1.into() });
    }

    #[test]
    #[should_panic(expected = "cannot sync issue 1 while it is Synced")]
    fn synced_issue_sync_issue_panics() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });

        store.consume_action(Action::SyncIssue {
            issue: sample_issue(1, "server issue", 1.into(), None, None, None, 0),
        });
    }

    #[test]
    fn cancel_issue_upload_returns_issue_to_edited_and_retains_diffs() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });
        store.consume_action(Action::StartIssueUpload { id: 1.into() });

        store.consume_action(Action::CancelIssueUpload { id: 1.into() });

        let (_, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(state, IssueState::Edited);
        assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
    }

    #[test]
    fn fail_issue_upload_returns_issue_to_edited_and_retains_diffs() {
        let mut store = Store::new();
        store.consume_action(Action::LoadIssue { id: 1.into() });
        store.consume_action(Action::UpdateIssue {
            id: 1.into(),
            body: "edited body".to_string(),
        });
        store.consume_action(Action::StartIssueUpload { id: 1.into() });

        store.consume_action(Action::FailIssueUpload { id: 1.into() });

        let (_, state) = store.get_issue(1).expect("issue should be loaded");
        assert_eq!(state, IssueState::Edited);
        assert_eq!(store.get_issue_property_diffs(IssueId::new(1)).len(), 1);
    }

    #[test]
    fn sync_issue_replaces_issue_clears_diffs_and_marks_synced() {
        let mut store = Store::new();
        store.consume_action(Action::SyncIssue {
            issue: sample_issue(9, "server issue before edit", 1.into(), None, None, None, 0),
        });
        store.consume_action(Action::UpdateIssue {
            id: 9.into(),
            body: "local edit".to_string(),
        });

        store.consume_action(Action::SyncIssue {
            issue: sample_issue(
                9,
                "server issue after upload",
                1.into(),
                None,
                None,
                None,
                0,
            ),
        });

        let (issue, state) = store.get_issue(9).expect("issue should be synced");
        assert_eq!(issue.subject, "server issue after upload");
        assert_eq!(issue.description, "body");
        assert_eq!(state, IssueState::Synced);
        assert!(store.get_issue_property_diffs(IssueId::new(9)).is_empty());
    }

    #[test]
    fn upload_success_sync_issue_replaces_issue_clears_diffs_and_marks_synced() {
        let mut store = Store::new();
        store.consume_action(Action::SyncIssue {
            issue: sample_issue(9, "server issue before edit", 1.into(), None, None, None, 0),
        });
        store.consume_action(Action::UpdateIssue {
            id: 9.into(),
            body: "local edit".to_string(),
        });
        store.consume_action(Action::StartIssueUpload { id: 9.into() });

        store.consume_action(Action::SyncIssue {
            issue: sample_issue(
                9,
                "server issue after upload",
                1.into(),
                None,
                None,
                None,
                0,
            ),
        });

        let (issue, state) = store.get_issue(9).expect("issue should be synced");
        assert_eq!(issue.subject, "server issue after upload");
        assert_eq!(issue.description, "body");
        assert_eq!(state, IssueState::Synced);
        assert!(store.get_issue_property_diffs(IssueId::new(9)).is_empty());
    }

    #[test]
    fn sync_users_replaces_users() {
        let mut store = Store::new();

        store.consume_action(Action::SyncUsers {
            users: vec![User {
                id: UserId::new(1001),
                name: "redmine user".to_string(),
            }],
        });

        assert_eq!(store.get_users().len(), 1);
        assert_eq!(
            store
                .get_user(UserId::new(1001))
                .expect("user should be synced")
                .name,
            "redmine user"
        );
    }

    #[test]
    fn sync_issue_statuses_replaces_issue_statuses() {
        let mut store = Store::new();

        store.consume_action(Action::SyncIssueStatuses {
            issue_statuses: vec![IssueStatus {
                id: IssueStatusId::new(10),
                name: "redmine status".to_string(),
                is_closed: false,
            }],
        });

        assert_eq!(store.get_issue_statuses().len(), 1);
        assert_eq!(
            store.get_issue_status(IssueStatusId::new(10)).name,
            "redmine status"
        );
    }

    #[test]
    fn sync_priorities_replaces_priorities() {
        let mut store = Store::new();

        store.consume_action(Action::SyncPriorities {
            priorities: vec![Priority {
                id: PriorityId::new(20),
                name: "redmine priority".to_string(),
            }],
        });

        assert_eq!(store.get_priorities().len(), 1);
        assert_eq!(
            store
                .get_priority(PriorityId::new(20))
                .expect("priority should be synced")
                .name,
            "redmine priority"
        );
    }

    #[test]
    fn sync_projects_replaces_projects() {
        let mut store = Store::new();

        store.consume_action(Action::SyncProjects {
            projects: vec![Project {
                id: ProjectId::new(30),
                name: "redmine project".to_string(),
            }],
        });

        assert_eq!(store.get_projects().len(), 1);
        assert_eq!(
            store
                .get_project(ProjectId::new(30))
                .expect("project should be synced")
                .name,
            "redmine project"
        );
    }

    #[test]
    fn sync_trackers_replaces_trackers() {
        let mut store = Store::new();

        store.consume_action(Action::SyncTrackers {
            trackers: vec![Tracker {
                id: TrackerId::new(40),
                name: "redmine tracker".to_string(),
            }],
        });

        assert_eq!(store.get_trackers().len(), 1);
        assert_eq!(
            store
                .get_tracker(TrackerId::new(40))
                .expect("tracker should be synced")
                .name,
            "redmine tracker"
        );
    }

    #[test]
    fn sync_target_versions_replaces_target_versions() {
        let mut store = Store::new();

        store.consume_action(Action::SyncTargetVersions {
            target_versions: vec![TargetVersion {
                id: TargetVersionId::new(50),
                name: "redmine version".to_string(),
            }],
        });

        assert_eq!(store.get_target_versions().len(), 1);
        assert_eq!(
            store
                .get_target_version(TargetVersionId::new(50))
                .expect("target version should be synced")
                .name,
            "redmine version"
        );
    }

    #[test]
    fn sync_categories_replaces_categories() {
        let mut store = Store::new();

        store.consume_action(Action::SyncCategories {
            categories: vec![Category {
                id: CategoryId::new(60),
                name: "redmine category".to_string(),
            }],
        });

        assert_eq!(store.get_categories().len(), 1);
        assert_eq!(
            store
                .get_category(CategoryId::new(60))
                .expect("category should be synced")
                .name,
            "redmine category"
        );
    }

    #[test]
    fn sync_time_entity_activities_replaces_time_entity_activities() {
        let mut store = Store::new();

        store.consume_action(Action::SyncTimeEntityActivities {
            time_entity_activities: vec![TimeEntityActivity {
                id: TimeEntityActivityId::new(70),
                name: "redmine activity".to_string(),
                is_default: true,
            }],
        });

        assert_eq!(store.get_time_entity_activities().len(), 1);
        assert_eq!(
            store
                .get_time_entity_activities()
                .get(&TimeEntityActivityId::new(70))
                .expect("time entity activity should be synced")
                .name,
            "redmine activity"
        );
    }
}
