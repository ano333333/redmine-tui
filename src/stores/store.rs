use std::collections::{HashMap, VecDeque};

use std::num::NonZeroUsize;

use super::issue_journals::JournalAction;
use super::issue_store::{IssueAction, IssueFetchState, IssueState, IssueStore};
use super::journal_state::{
    DeletedJournalEntry, LocalJournalEntry, RemoteJournalUploadConflict, RemoteJournalView,
};
use super::notice_store::{Notice, NoticeAction, NoticeStore};
use super::project_issues_store::{
    ProjectIssuesAction, ProjectIssuesPageState, ProjectIssuesStore,
};
use crate::entities::{
    Category, Issue, IssueAggregate, IssueChild, IssueStatus, IssueView, Priority, Project,
    TargetVersion, TimeEntityActivity, Tracker, User,
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
    /// 時間経過に依存する全Storeへ、前回更新からの同じ経過時間を適用する。
    pub fn update_store(&mut self, tick: chrono::Duration) {
        self.store.update(tick);
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
        match action {
            Action::Issue(action) => self.issue_store.consume_action(action),
            Action::IssueFetchSucceeded {
                id,
                issue,
                children,
            } => self.issue_store.complete_fetch(id, issue, children),
            Action::ProjectIssues(action) => self.project_issues_store.consume_action(action),
            Action::Journal(action) => self.issue_store.consume_journal_action(action),
            Action::Notice(action) => self.notice_store.consume_action(action),
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

    /// # Panics
    ///
    /// 未登録・取得中・取得失敗のIssueを指定した場合にpanicする。
    /// 取得済みかどうかが不明な経路では、先に`try_get_issue_state`で確認する。
    #[track_caller]
    pub fn get_issue(&self, issue_id: impl Into<IssueId>) -> (IssueView<'_>, IssueState) {
        self.issue_store.get_issue(issue_id)
    }

    /// 未登録・取得中・取得失敗のIssueではどれも`None`を返す。
    pub fn try_get_issue_state(&self, issue_id: impl Into<IssueId>) -> Option<IssueState> {
        self.issue_store.try_get_issue_state(issue_id)
    }

    /// 未登録と取得済みのIssueではどちらも`None`を返すため、
    /// 未登録の判定には`try_get_issue_state`も併用する。
    pub fn try_get_issue_fetch_state(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<IssueFetchState> {
        self.issue_store.try_get_issue_fetch_state(issue_id)
    }

    /// 詳細取得で得た子Issueの一覧を返す。子一覧を取得していないIssueでは空のsliceを返す。
    pub fn get_issue_children(&self, issue_id: impl Into<IssueId>) -> &[IssueChild] {
        self.issue_store.get_issue_children(issue_id)
    }

    /// 優先度を更新できるかを返す。
    pub fn is_issue_priority_editable(&self, issue_id: impl Into<IssueId>) -> bool {
        self.get_issue_children(issue_id).is_empty()
    }

    /// 開始日・期日を更新できるかを返す。
    pub fn are_issue_dates_editable(&self, issue_id: impl Into<IssueId>) -> bool {
        self.get_issue_children(issue_id).is_empty()
    }

    /// 進捗率を更新できるかを返す。
    pub fn is_issue_done_ratio_editable(&self, issue_id: impl Into<IssueId>) -> bool {
        self.get_issue_children(issue_id).is_empty()
    }

    pub fn get_issues(&self) -> impl Iterator<Item = (IssueView<'_>, IssueState)> {
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
    /// Issueが取得済みでない場合は空のVecを返す。
    pub fn get_remote_journals(&self, issue_id: impl Into<IssueId>) -> Vec<RemoteJournalView<'_>> {
        self.issue_store.get_remote_journals(issue_id)
    }

    /// Issueに紐づく0件または1件のLocal Journalを返す。
    pub fn try_get_local_journal(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<&LocalJournalEntry> {
        self.issue_store.try_get_local_journal(issue_id)
    }

    /// # Panics
    ///
    /// Issueが未登録の場合、またはLocal Journalを持たない場合にpanicする。
    #[track_caller]
    pub fn get_local_journal(&self, issue_id: impl Into<IssueId>) -> &LocalJournalEntry {
        let issue_id = issue_id.into();
        match self.issue_store.try_get_local_journal(issue_id) {
            Some(entry) => entry,
            None => panic!("local journal is not registered for issue {issue_id}"),
        }
    }

    /// 取得結果から消えた編集中のJournalを、退避した順に返す。
    ///
    /// Issueが取得済みでない場合は空のsliceを返す。
    pub fn get_deleted_journals(&self, issue_id: impl Into<IssueId>) -> &[DeletedJournalEntry] {
        self.issue_store.get_deleted_journals(issue_id)
    }

    /// # Panics
    ///
    /// 指定した元IDの退避データが登録されていない場合にpanicする。
    #[track_caller]
    pub fn get_deleted_journal(
        &self,
        issue_id: impl Into<IssueId>,
        original_id: impl Into<JournalId>,
    ) -> &DeletedJournalEntry {
        let issue_id = issue_id.into();
        let original_id = original_id.into();
        match self
            .get_deleted_journals(issue_id)
            .iter()
            .find(|entry| entry.original_id == original_id)
        {
            Some(entry) => entry,
            None => panic!("deleted journal {original_id} is not registered for issue {issue_id}"),
        }
    }

    /// 対象IssueのRemote Journal、Local Journal、退避したJournalのいずれかがupload中かを返す。
    ///
    /// 取得済みでないIssue、およびJournalがすべて待機中のIssueでは`false`を返す。
    /// usecaseから同一Issue内のJournal upload排他を検査するために使用する。
    pub fn has_uploading_journal(&self, issue_id: impl Into<IssueId>) -> bool {
        self.issue_store.has_uploading_journal(issue_id)
    }

    /// Issueに登録されているRemote Journalを返す。
    ///
    /// # Panics
    ///
    /// 指定したIssueに指定したRemote Journalが登録されていない場合にpanicする。
    #[track_caller]
    pub fn get_remote_journal(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> RemoteJournalView<'_> {
        self.issue_store.get_remote_journal(issue_id, journal_id)
    }

    /// 競合解決に必要なRemote Journalの編集差分とサーバー値を返す。
    ///
    /// 対象が未登録の場合、または競合中でない場合は`None`を返す。
    pub fn try_get_remote_journal_upload_conflict(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> Option<(&JournalNotesDiff, &RemoteJournalUploadConflict)> {
        self.issue_store
            .try_get_remote_journal_upload_conflict(issue_id, journal_id)
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

    /// 起動時snapshotに対象statusが含まれない場合は`None`を返す。
    /// 呼び出し側はこの欠損を通常状態として扱い、用途に応じたfallbackを行う。
    pub fn get_issue_status(&self, issue_status_id: IssueStatusId) -> Option<&IssueStatus> {
        self.issue_statuses.get(&issue_status_id)
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

    pub fn get_target_versions(&self, project_id: ProjectId) -> Vec<&TargetVersion> {
        self.target_versions
            .values()
            .filter(|target_version| target_version.project_id == project_id)
            .collect()
    }

    pub fn get_target_version(&self, target_version_id: TargetVersionId) -> Option<&TargetVersion> {
        self.target_versions.get(&target_version_id)
    }

    pub fn get_categories(&self, project_id: ProjectId) -> Vec<&Category> {
        self.categories
            .values()
            .filter(|category| category.project_id == project_id)
            .collect()
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
    pub fn try_get_issue_upload_conflict(
        &self,
        id: IssueId,
    ) -> Option<(&IssueAggregate, &[IssuePropertyDiff])> {
        self.issue_store.try_get_issue_upload_conflict(id)
    }

    pub fn try_get_issue_upload_failure(&self, id: IssueId) -> Option<&str> {
        self.issue_store.try_get_issue_upload_failure(id)
    }

    /// PUT後の確認の取得に失敗したSyncedのIssueでだけ、失敗の理由を返す。
    pub fn try_get_issue_confirmation_failure(&self, id: IssueId) -> Option<&str> {
        self.issue_store.try_get_issue_confirmation_failure(id)
    }

    /// 表示中のnoticeを追加順で返す。
    pub fn get_notices(&self) -> &[Notice] {
        self.notice_store.notices()
    }

    /// 前回更新からの経過時間を内部状態へ累積する。
    ///
    /// この更新で表示期限ちょうどに達したnoticeも破棄する。
    pub fn update(&mut self, tick: chrono::Duration) {
        self.notice_store.update(tick);
    }
}

pub enum Action {
    Issue(IssueAction),
    /// Issue詳細の初回取得結果を、Issue本体とJournalが揃った状態で一度に反映する。
    ///
    /// 対象IssueがFetching以外の場合、要求IDと取得したIssueのIDが異なる場合、
    /// Journalの所有Issueが異なる・ID重複・他Issueに登録済みの場合はpanicし、Storeを更新しない。
    IssueFetchSucceeded {
        id: IssueId,
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
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
    use crate::entities::Journal;
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::id::EntityIdValue;
    use crate::vos::{
        CategoryId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TimeEntityActivityId,
        TrackerId, UserId,
    };

    #[test]
    fn dispatcher_accepts_issue_action_directly() {
        let id = IssueId::new(1);
        let mut dispatcher = Dispatcher::new();

        crate::test_support::dispatch_loaded_issue(
            &mut dispatcher,
            sample_issue_aggregate(1, "issue", 1.into(), None, None, None, 0),
        );

        assert!(dispatcher.store().try_get_issue_state(id).is_some());
    }

    #[test]
    #[should_panic(expected = "cannot start issue upload while a journal of issue 1 is uploading")]
    fn issue_upload_start_panics_while_a_journal_of_the_issue_is_uploading() {
        let id = IssueId::new(1);
        let mut store = Store::new();
        crate::test_support::load_issue(
            &mut store,
            sample_issue_aggregate(1, "issue", 1.into(), None, None, None, 0),
        );
        store.consume_action(JournalAction::CreateLocal { issue_id: id }.into());
        store.consume_action(JournalAction::StartLocalUpload { issue_id: id }.into());
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
        crate::test_support::load_issue(
            &mut store,
            sample_issue_aggregate(1, "issue", 1.into(), None, None, None, 0),
        );
        store.consume_action(JournalAction::CreateLocal { issue_id: id }.into());
        store.consume_action(
            IssueAction::UpdateDescription {
                id,
                body: "edited".to_string(),
            }
            .into(),
        );

        store.consume_action(IssueAction::StartUpload { id }.into());

        assert_eq!(store.try_get_issue_state(id), Some(IssueState::Uploading));
    }

    fn fetched_journal(id: u16, issue_id: u16) -> Journal {
        Journal {
            id: JournalId::new(id),
            issue_id: IssueId::new(issue_id),
            user: "alice".to_string(),
            updated_on: None,
            details: vec![],
            notes: format!("notes {id}"),
        }
    }

    fn fetch_succeeded(id: u16, issue_id: u16, journals: Vec<Journal>) -> Action {
        let mut issue = sample_issue_aggregate(issue_id, "fetched", 1.into(), None, None, None, 0);
        issue.journals = journals;
        Action::IssueFetchSucceeded {
            id: IssueId::new(id),
            issue,
            children: vec![],
        }
    }

    fn consume_expecting_panic(store: &mut Store, action: Action) {
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.consume_action(action);
        }))
        .is_err();
        assert!(panicked, "invalid fetch completion must be rejected");
    }

    #[test]
    fn issue_fetch_success_registers_the_issue_and_its_journals_in_one_action() {
        let mut store = Store::new();
        store.consume_action(IssueAction::StartFetching { id: 1.into() }.into());

        store.consume_action(fetch_succeeded(
            1,
            1,
            vec![fetched_journal(10, 1), fetched_journal(11, 1)],
        ));

        assert_eq!(store.try_get_issue_state(1), Some(IssueState::Synced));
        let journal_ids: Vec<u16> = store
            .get_remote_journals(1)
            .iter()
            .map(|entry| entry.journal.id.get())
            .collect();
        assert_eq!(journal_ids, vec![10, 11]);
    }

    #[test]
    fn issue_fetch_success_without_fetching_state_leaves_journals_unregistered() {
        let mut store = Store::new();

        consume_expecting_panic(
            &mut store,
            fetch_succeeded(1, 1, vec![fetched_journal(10, 1)]),
        );

        assert!(store.get_remote_journals(1).is_empty());
        assert_eq!(store.try_get_issue_state(1), None);
    }

    #[test]
    fn issue_fetch_success_with_mismatched_issue_leaves_journals_unregistered() {
        let mut store = Store::new();
        store.consume_action(IssueAction::StartFetching { id: 1.into() }.into());

        consume_expecting_panic(
            &mut store,
            fetch_succeeded(1, 2, vec![fetched_journal(10, 1)]),
        );

        assert!(store.get_remote_journals(1).is_empty());
        assert_eq!(
            store.try_get_issue_fetch_state(1),
            Some(IssueFetchState::Fetching)
        );
    }

    #[test]
    fn issue_fetch_success_with_duplicate_journals_keeps_the_issue_fetching() {
        let mut store = Store::new();
        store.consume_action(IssueAction::StartFetching { id: 1.into() }.into());

        consume_expecting_panic(
            &mut store,
            fetch_succeeded(1, 1, vec![fetched_journal(10, 1), fetched_journal(10, 1)]),
        );

        assert_eq!(
            store.try_get_issue_fetch_state(1),
            Some(IssueFetchState::Fetching)
        );
        assert!(store.get_remote_journals(1).is_empty());
    }

    #[test]
    fn issue_fetch_success_with_a_journal_of_another_issue_keeps_the_issue_fetching() {
        let mut store = Store::new();
        store.consume_action(IssueAction::StartFetching { id: 1.into() }.into());

        consume_expecting_panic(
            &mut store,
            fetch_succeeded(1, 1, vec![fetched_journal(10, 2)]),
        );

        assert_eq!(
            store.try_get_issue_fetch_state(1),
            Some(IssueFetchState::Fetching)
        );
        assert!(store.get_remote_journals(1).is_empty());
    }

    #[test]
    fn get_target_versions_filters_by_project_id() {
        let mut store = Store::new();

        store.consume_action(Action::SyncTargetVersions {
            target_versions: vec![
                TargetVersion {
                    id: TargetVersionId::new(1),
                    name: "version a".to_string(),
                    project_id: ProjectId::new(1),
                },
                TargetVersion {
                    id: TargetVersionId::new(2),
                    name: "version b".to_string(),
                    project_id: ProjectId::new(2),
                },
                TargetVersion {
                    id: TargetVersionId::new(3),
                    name: "version c".to_string(),
                    project_id: ProjectId::new(1),
                },
            ],
        });

        let matched = store.get_target_versions(ProjectId::new(1));
        let matched_ids: Vec<u16> = matched.iter().map(|version| version.id.get()).collect();
        assert_eq!(matched_ids.len(), 2);
        assert!(matched_ids.contains(&1));
        assert!(matched_ids.contains(&3));

        let unmatched = store.get_target_versions(ProjectId::new(99));
        assert!(unmatched.is_empty());
    }

    #[test]
    fn get_categories_filters_by_project_id() {
        let mut store = Store::new();

        store.consume_action(Action::SyncCategories {
            categories: vec![
                Category {
                    id: CategoryId::new(1),
                    name: "category a".to_string(),
                    project_id: ProjectId::new(1),
                },
                Category {
                    id: CategoryId::new(2),
                    name: "category b".to_string(),
                    project_id: ProjectId::new(2),
                },
                Category {
                    id: CategoryId::new(3),
                    name: "category c".to_string(),
                    project_id: ProjectId::new(1),
                },
            ],
        });

        let matched = store.get_categories(ProjectId::new(1));
        let matched_ids: Vec<u16> = matched.iter().map(|category| category.id.get()).collect();
        assert_eq!(matched_ids.len(), 2);
        assert!(matched_ids.contains(&1));
        assert!(matched_ids.contains(&3));

        let unmatched = store.get_categories(ProjectId::new(99));
        assert!(unmatched.is_empty());
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
    fn sync_issue_statuses_replaces_entries_and_lookup_handles_known_and_unknown_ids() {
        let mut store = Store::new();

        store.consume_action(Action::SyncIssueStatuses {
            issue_statuses: vec![IssueStatus {
                id: IssueStatusId::new(10),
                name: "redmine status".to_string(),
                is_closed: false,
            }],
        });

        assert_eq!(store.get_issue_statuses().len(), 1);
        let found = store.get_issue_status(IssueStatusId::new(10));

        assert!(matches!(found, Some(status) if status.name == "redmine status"));
        assert!(store.get_issue_status(IssueStatusId::new(99)).is_none());
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
                project_id: ProjectId::new(1),
            }],
        });

        assert_eq!(store.get_target_versions(ProjectId::new(1)).len(), 1);
        assert!(store.get_target_versions(ProjectId::new(2)).is_empty());
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
                project_id: ProjectId::new(1),
            }],
        });

        assert_eq!(store.get_categories(ProjectId::new(1)).len(), 1);
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
