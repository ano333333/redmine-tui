use std::collections::{HashMap, HashSet, VecDeque};

use std::num::NonZeroUsize;

use super::issue_store::{IssueAction, IssueState, IssueStore};
use super::journal_store::{JournalAction, JournalEntry, JournalStore};
use super::project_issues_store::{
    ProjectIssuesAction, ProjectIssuesPageState, ProjectIssuesStore,
};
use crate::entities::{
    Category, Issue, IssueAggregate, IssueStatus, Journal, Priority, Project, TargetVersion,
    TimeEntityActivity, Tracker, User,
};
use crate::libs::yaml::parse_journal_yaml;
use crate::vos::{
    CategoryId, IssueId, IssuePropertyDiff, IssueStatusId, JournalId, JournalKey, LocalJournalId,
    PriorityId, ProjectId, TargetVersionId, TimeEntityActivityId, TrackerId, UserId,
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
    pub fn dispatch(&mut self, action: impl Into<Action>) {
        self.actions.push_back(action.into());
    }
    pub fn consume_actinos_len(&self) -> usize {
        self.actions.len()
    }
    pub fn consume_action(&mut self) {
        if let Some(action) = self.actions.pop_front() {
            self.store.consume_action(action);
        }
    }

    /// Reserves an ID for a Local Journal without creating the Journal itself.
    ///
    /// Pass the returned ID to [`JournalAction::CreateLocal`] through this same
    /// Dispatcher. IDs increase monotonically and are never reused. If creation
    /// is abandoned, the reserved ID remains unused and becomes a permitted gap.
    ///
    /// The ID does not encode Store provenance. Mixing IDs between different
    /// Dispatcher/Store instances is unsupported and must be avoided by callers.
    pub fn new_local_journal_id(&mut self) -> LocalJournalId {
        self.store.new_local_journal_id()
    }
}

pub enum JournalState {
    Synced,
    Updated,
}

pub struct Store {
    issue_store: IssueStore,
    project_issues_store: ProjectIssuesStore,
    journal_store: JournalStore,
    next_local_journal_id: u64,
    issued_local_journal_ids: HashSet<LocalJournalId>,
    used_local_journal_ids: HashSet<LocalJournalId>,
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
            issue_store: IssueStore::new(),
            project_issues_store: ProjectIssuesStore::new(),
            journal_store: JournalStore::new(),
            next_local_journal_id: 1,
            issued_local_journal_ids: HashSet::new(),
            used_local_journal_ids: HashSet::new(),
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
            Action::Issue(action) => self.issue_store.consume_action(action),
            Action::ProjectIssues(action) => self.project_issues_store.consume_action(action),
            Action::Journal(action) => {
                if let JournalAction::CreateLocal { id, .. } = &action {
                    if !self.issued_local_journal_ids.contains(id) {
                        panic!("local journal ID was not issued by this Store");
                    }
                    if self.used_local_journal_ids.contains(id) {
                        panic!("local journal ID has already been used");
                    }
                }
                let created_id = match &action {
                    JournalAction::CreateLocal { id, .. } => Some(*id),
                    JournalAction::EditLocalNotes { .. } => None,
                };
                self.journal_store.consume_action(action);
                if let Some(id) = created_id {
                    self.used_local_journal_ids.insert(id);
                }
            }
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

    fn new_local_journal_id(&mut self) -> LocalJournalId {
        let id = LocalJournalId::new(self.next_local_journal_id);
        self.next_local_journal_id = self
            .next_local_journal_id
            .checked_add(1)
            .expect("local journal ID space exhausted");
        self.issued_local_journal_ids.insert(id);
        id
    }

    pub fn get_issue(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<(&IssueAggregate, &IssueState)> {
        self.issue_store.get_issue(issue_id)
    }

    pub fn get_issue_state(&self, issue_id: impl Into<IssueId>) -> Option<&IssueState> {
        self.issue_store.get_issue_state(issue_id)
    }

    pub fn get_issues(&self) -> &HashMap<IssueId, IssueAggregate> {
        self.issue_store.get_issues()
    }

    pub fn get_project_issues_page_state(
        &self,
        project_id: impl Into<ProjectId>,
        page: NonZeroUsize,
    ) -> Option<&ProjectIssuesPageState> {
        self.project_issues_store
            .page_state(project_id.into(), page)
    }

    pub fn get_project_issues(
        &self,
        project_id: impl Into<ProjectId>,
        page: NonZeroUsize,
    ) -> Option<&[Issue]> {
        self.project_issues_store.issues(project_id.into(), page)
    }

    pub fn get_issue_property_diffs(&self, issue_id: impl Into<IssueId>) -> &[IssuePropertyDiff] {
        self.issue_store.get_issue_property_diffs(issue_id)
    }

    pub fn get_journal(
        &self,
        journal_id: impl Into<JournalId>,
    ) -> Option<&(Journal, JournalState)> {
        self.journals.get(&journal_id.into())
    }

    pub fn get_journal_entry(&self, key: JournalKey) -> Option<&JournalEntry> {
        self.journal_store.entry(key)
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
    pub fn get_issue_upload_conflict(
        &self,
        id: IssueId,
    ) -> Option<(&IssueAggregate, &[IssuePropertyDiff])> {
        self.issue_store.get_issue_upload_conflict(id)
    }
}

pub enum Action {
    Issue(IssueAction),
    ProjectIssues(ProjectIssuesAction),
    Journal(JournalAction),
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
    LoadJournal {
        id: JournalId,
    },
    UpdateJournal {
        id: JournalId,
        notes: String,
    },
}

impl From<IssueAction> for Action {
    fn from(action: IssueAction) -> Self {
        Self::Issue(action)
    }
}

impl From<ProjectIssuesAction> for Action {
    fn from(action: ProjectIssuesAction) -> Self {
        Self::ProjectIssues(action)
    }
}

impl From<JournalAction> for Action {
    fn from(action: JournalAction) -> Self {
        Self::Journal(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sync_fixture_entities;
    use crate::vos::{
        CategoryId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TimeEntityActivityId,
        TrackerId, UserId,
    };

    #[test]
    fn dispatcher_accepts_issue_action_directly() {
        let id = IssueId::new(1);
        let mut dispatcher = Dispatcher::new();

        dispatcher.dispatch(IssueAction::Load { id });
        dispatcher.consume_action();

        assert!(dispatcher.store().get_issue(id).is_some());
    }

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
