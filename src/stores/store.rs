use std::collections::{HashMap, VecDeque};

use std::num::NonZeroUsize;

use super::issue_store::{IssueAction, IssueState, IssueStore};
use super::journal_state::{LocalJournalEntry, RemoteJournalEntry, RemoteJournalUploadConflict};
use super::journal_store::{JournalAction, JournalStore};
use super::notice_store::{Notice, NoticeAction, NoticeStore};
use super::project_issues_store::{
    ProjectIssuesAction, ProjectIssuesPageState, ProjectIssuesStore,
};
use crate::entities::{
    Category, Issue, IssueAggregate, IssueStatus, Priority, Project, TargetVersion,
    TimeEntityActivity, Tracker, User,
};
use crate::vos::{
    CategoryId, IssueId, IssuePropertyDiff, IssueStatusId, JournalId, JournalNotesDiff, PriorityId,
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
    pub fn dispatch(&mut self, action: impl Into<Action>) {
        self.actions.push_back(action.into());
    }
    pub fn consume_actinos_len(&self) -> usize {
        self.actions.len()
    }
    /// 時間経過に依存する全Storeの状態を、呼び出し側が取得した同一時刻で更新する。
    pub fn update_store(&mut self, now: chrono::DateTime<chrono::Local>) {
        self.store.update(now);
    }
    pub fn consume_action(&mut self) {
        if let Some(action) = self.actions.pop_front() {
            self.store.consume_action(action);
        }
    }
}

pub struct Store {
    issue_store: IssueStore,
    project_issues_store: ProjectIssuesStore,
    journal_store: JournalStore,
    notice_store: NoticeStore,
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
            notice_store: NoticeStore::new(),
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
        // IssueStoreとJournalStoreは互いを参照しないため、両者をまたぐupload排他は
        // Action処理の共通入口で検査し、検査を通過したActionだけを子Storeへ委譲する。
        match action {
            Action::Issue(IssueAction::StartUpload { id }) => {
                if matches!(
                    self.issue_store.get_issue_state(id),
                    Some(IssueState::Edited)
                ) {
                    assert!(
                        !self.journal_store.has_uploading_journal(id),
                        "cannot start issue upload while a journal of issue {id} is uploading"
                    );
                }
                self.issue_store
                    .consume_action(IssueAction::StartUpload { id });
            }
            Action::Issue(action) => self.issue_store.consume_action(action),
            Action::ProjectIssues(action) => self.project_issues_store.consume_action(action),
            Action::Journal(JournalAction::StartLocalUpload { issue_id }) => {
                assert!(
                    !matches!(
                        self.issue_store.get_issue_state(issue_id),
                        Some(IssueState::Uploading)
                    ),
                    "cannot start local journal upload while issue {issue_id} is uploading"
                );
                self.journal_store
                    .consume_action(JournalAction::StartLocalUpload { issue_id });
            }
            Action::Journal(JournalAction::StartRemoteUpload {
                issue_id,
                journal_id,
            }) => {
                assert!(
                    !matches!(
                        self.issue_store.get_issue_state(issue_id),
                        Some(IssueState::Uploading)
                    ),
                    "cannot start remote journal upload while issue {issue_id} is uploading"
                );
                self.journal_store
                    .consume_action(JournalAction::StartRemoteUpload {
                        issue_id,
                        journal_id,
                    });
            }
            Action::Journal(action) => self.journal_store.consume_action(action),
            Action::Notice(action) => self.notice_store.consume_action(action),
            // main loop が直接処理する終了通知であり、Store の状態には反映しない。
            Action::WorkerPanicked { .. } => {}
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
        }
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

    /// IssueのRemote Journalを保持順に返す。
    ///
    /// Issueが未登録の場合は空のsliceを返す。
    pub fn get_remote_journals(&self, issue_id: impl Into<IssueId>) -> &[RemoteJournalEntry] {
        self.journal_store.get_remote_journals(issue_id)
    }

    /// Issueに紐づく0件または1件のLocal Journalを返す。
    pub fn get_local_journal(&self, issue_id: impl Into<IssueId>) -> Option<&LocalJournalEntry> {
        self.journal_store.get_local_journal(issue_id)
    }

    /// 対象IssueのRemote JournalまたはLocal Journalがupload中かを返す。
    ///
    /// Journalが未登録のIssue、およびJournalがすべて待機中のIssueでは`false`を返す。
    /// usecaseから同一Issue内のJournal upload排他を検査するために使用する。
    pub fn has_uploading_journal(&self, issue_id: impl Into<IssueId>) -> bool {
        self.journal_store.has_uploading_journal(issue_id)
    }

    /// Issueに登録されているRemote Journalを返す。
    ///
    /// # Panics
    ///
    /// 指定したIssueに指定したRemote Journalが登録されていない場合にpanicする。
    pub fn get_remote_journal(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> &RemoteJournalEntry {
        self.journal_store.get_remote_journal(issue_id, journal_id)
    }

    /// 競合解決に必要なRemote Journalの編集差分とサーバー値を返す。
    ///
    /// 対象が未登録の場合、または競合中でない場合は`None`を返す。
    pub fn get_remote_journal_upload_conflict(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> Option<(&JournalNotesDiff, &RemoteJournalUploadConflict)> {
        self.journal_store
            .get_remote_journal_upload_conflict(issue_id, journal_id)
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

    /// 表示中のnoticeを追加順で返す。
    pub fn get_notices(&self) -> &[Notice] {
        self.notice_store.notices()
    }

    /// 時間経過に依存する内部状態を更新する。
    ///
    /// `now`ちょうどに表示期限を迎えたnoticeも破棄する。
    pub fn update(&mut self, now: chrono::DateTime<chrono::Local>) {
        self.notice_store.update(now);
    }
}

pub enum Action {
    Issue(IssueAction),
    ProjectIssues(ProjectIssuesAction),
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
    Journal(JournalAction),
    Notice(NoticeAction),
    /// worker task の panic を main loop へ伝え、プロセスを異常終了させる。
    WorkerPanicked {
        message: String,
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

impl From<NoticeAction> for Action {
    fn from(action: NoticeAction) -> Self {
        Self::Notice(action)
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
    #[should_panic(expected = "cannot start issue upload while a journal of issue 1 is uploading")]
    fn issue_upload_start_panics_while_a_journal_of_the_issue_is_uploading() {
        let id = IssueId::new(1);
        let mut store = Store::new();
        store.consume_action(JournalAction::CreateLocal { issue_id: id }.into());
        store.consume_action(JournalAction::StartLocalUpload { issue_id: id }.into());
        store.consume_action(IssueAction::Load { id }.into());
        store.consume_action(
            IssueAction::UpdateDescription {
                id,
                body: "edited".to_string(),
            }
            .into(),
        );

        store.consume_action(IssueAction::StartUpload { id }.into());
    }

    #[test]
    fn issue_upload_start_succeeds_while_the_issues_journals_are_idle() {
        let id = IssueId::new(1);
        let mut store = Store::new();
        store.consume_action(JournalAction::CreateLocal { issue_id: id }.into());
        store.consume_action(IssueAction::Load { id }.into());
        store.consume_action(
            IssueAction::UpdateDescription {
                id,
                body: "edited".to_string(),
            }
            .into(),
        );

        store.consume_action(IssueAction::StartUpload { id }.into());

        assert_eq!(store.get_issue_state(id), Some(&IssueState::Uploading));
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
