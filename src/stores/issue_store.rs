//! Issueの取得・編集・uploadに伴う状態を保持し、Actionで遷移させる。
//! entryのvariantで本体・差分・失敗・競合の保持可能な組合せを制限する。
//! Actionごとの遷移前提条件は受理時に検査する。

use std::collections::{HashMap, HashSet};

use super::issue_journals::{IssueJournalStates, JournalAction};
use super::journal_state::{
    DeletedJournalEntry, LocalJournalEntry, RemoteJournalState, RemoteJournalUploadConflict,
    RemoteJournalView,
};
use crate::entities::{IssueAggregate, IssueChild, IssueView, Journal};
use crate::vos::issue_property_diff::{
    IssueAssignedToIdDiff, IssueCategoryIdDiff, IssueDescriptionDiff, IssueDoneRatioDiff,
    IssueDueDateDiff, IssueEstimatedHoursDiff, IssuePriorityIdDiff, IssueProjectIdDiff,
    IssueStartDateDiff, IssueStatusIdDiff, IssueTargetVersionIdDiff, IssueTrackerIdDiff,
    fold_property_diffs,
};
use crate::vos::{
    CategoryId, IssueId, IssuePropertyDiff, IssueStatusId, JournalId, JournalNotesDiff, PriorityId,
    ProjectId, TargetVersionId, TrackerId, UserId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueState {
    Synced,
    Edited,
    Uploading,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueFetchState {
    Fetching,
    FetchFailed { message: String },
}

#[derive(Debug)]
enum IssueEntry {
    Fetching,
    FetchFailed {
        message: String,
    },
    Synced {
        issue: IssueAggregate,
        journal_states: IssueJournalStates,
    },
    Edited {
        issue: IssueAggregate,
        diffs: Vec<IssuePropertyDiff>,
        failure: Option<String>,
        journal_states: IssueJournalStates,
    },
    Uploading {
        issue: IssueAggregate,
        diffs: Vec<IssuePropertyDiff>,
        conflict: Option<IssueUploadConflict>,
        journal_states: IssueJournalStates,
    },
}

#[derive(Debug)]
struct IssueUploadConflict {
    server_issue: IssueAggregate,
    conflicts: Vec<IssuePropertyDiff>,
}

pub enum IssueAction {
    Sync {
        issue: IssueAggregate,
    },
    StartFetching {
        id: IssueId,
    },
    FetchFailed {
        id: IssueId,
        message: String,
    },
    StartUpload {
        id: IssueId,
    },
    CancelUpload {
        id: IssueId,
    },
    ClearUploadConflicts {
        id: IssueId,
    },
    FailUpload {
        id: IssueId,
        message: String,
    },
    /// 保存前の取得でIssue属性の競合を検出した結果を保持する。
    ///
    /// Issue属性の基準値は競合の解決後に取得し直すまで更新しない。取得したJournalと子一覧は
    /// この時点で取り込む。Uploading以外の状態の場合はpanicする。
    UploadConflictsDetected {
        server_issue: IssueAggregate,
        conflicts: Vec<IssuePropertyDiff>,
        children: Vec<IssueChild>,
    },
    /// Issue属性の保存が成功した後、取得したIssueを新しい基準値としてSyncedへ戻す。
    ///
    /// Journalの編集・下書き・退避データは取得したJournalを取り込みながら引き継ぐ。
    /// Uploading以外の状態の場合はpanicする。
    UploadSucceeded {
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    UpdateDescription {
        id: IssueId,
        body: String,
    },
    UpdateStatus {
        id: IssueId,
        status_id: IssueStatusId,
    },
    UpdateTracker {
        id: IssueId,
        tracker_id: TrackerId,
    },
    /// 対象バージョンとカテゴリーはproject単位のため、移動先で無効にならないよう未設定に戻す。
    UpdateProject {
        id: IssueId,
        project_id: ProjectId,
    },
    UpdatePriority {
        id: IssueId,
        priority_id: PriorityId,
    },
    UpdateAssignedTo {
        id: IssueId,
        assigned_to_id: Option<UserId>,
    },
    UpdateTargetVersion {
        id: IssueId,
        target_version_id: Option<TargetVersionId>,
    },
    UpdateCategory {
        id: IssueId,
        category_id: Option<CategoryId>,
    },
    UpdateDoneRatio {
        id: IssueId,
        done_ratio: u16,
    },
    UpdateEstimatedHours {
        id: IssueId,
        estimated_hours: Option<f64>,
    },
    UpdateStartDate {
        id: IssueId,
        start_date: Option<chrono::DateTime<chrono::Local>>,
    },
    UpdateDueDate {
        id: IssueId,
        due_date: Option<chrono::DateTime<chrono::Local>>,
    },
}

pub(super) struct IssueStore {
    entries: HashMap<IssueId, IssueEntry>,
    /// 詳細取得で得た子Issueの一覧。子Issue自身の詳細の取得状態とは独立して保持する。
    children: HashMap<IssueId, Vec<IssueChild>>,
}

impl IssueStore {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            children: HashMap::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: IssueAction) {
        match action {
            IssueAction::Sync { mut issue } => {
                let id = issue.issue.id;
                match self.entries.get(&id) {
                    None | Some(IssueEntry::Edited { .. } | IssueEntry::Uploading { .. }) => {}
                    entry => panic!(
                        "cannot sync issue {id} while it is {}",
                        Self::entry_state_name(entry)
                    ),
                }
                self.assert_fetched_journals_are_valid(id, &issue.journals);
                // Issue属性の保存が完了しても、Journalの編集と下書きは引き継ぐ。
                let journal_states = match self.entries.remove(&id) {
                    None => IssueJournalStates::synced(&issue.journals),
                    Some(
                        IssueEntry::Edited {
                            issue: current,
                            mut journal_states,
                            ..
                        }
                        | IssueEntry::Uploading {
                            issue: current,
                            mut journal_states,
                            ..
                        },
                    ) => {
                        let fetched = std::mem::take(&mut issue.journals);
                        issue.journals = journal_states.merge_fetched(current.journals, fetched);
                        journal_states
                    }
                    Some(_) => unreachable!("state check guarantees Edited or Uploading"),
                };
                self.entries.insert(
                    id,
                    IssueEntry::Synced {
                        issue,
                        journal_states,
                    },
                );
            }
            IssueAction::StartFetching { id } => {
                // UIとfetch usecaseは未登録またはFetchFailedの場合にだけこのActionを発行する。
                // Fetchingや取得済み状態への着弾は呼び出し側の不変条件違反として拒否する。
                match self.entries.get(&id) {
                    None | Some(IssueEntry::FetchFailed { .. }) => {}
                    entry => panic!(
                        "cannot start fetching issue {id} while it is {}",
                        Self::entry_state_name(entry)
                    ),
                }
                self.entries.insert(id, IssueEntry::Fetching);
            }
            IssueAction::FetchFailed { id, message } => {
                // 新しい同期結果やローカル編集を遅延した失敗で破棄しないよう、
                // Fetching以外への着弾は制御破綻として拒否する。
                match self.entries.get(&id) {
                    Some(IssueEntry::Fetching) => {}
                    None => panic!("fetch failed for issue {id} without an issue state"),
                    entry => panic!(
                        "fetch failed while issue {id} is {}",
                        Self::entry_state_name(entry)
                    ),
                }
                self.entries.insert(id, IssueEntry::FetchFailed { message });
            }
            IssueAction::StartUpload { id } => {
                let entry = self.entries.get(&id);
                match entry {
                    Some(IssueEntry::Edited { journal_states, .. }) => assert!(
                        !journal_states.has_uploading(),
                        "cannot start issue upload while a journal of issue {id} is uploading"
                    ),
                    _ => panic!(
                        "cannot start issue upload while issue {id} is {}",
                        Self::entry_state_name(entry)
                    ),
                }
                match self.entries.remove(&id).unwrap() {
                    IssueEntry::Edited {
                        issue,
                        diffs,
                        journal_states,
                        ..
                    } => {
                        self.entries.insert(
                            id,
                            IssueEntry::Uploading {
                                issue,
                                diffs,
                                conflict: None,
                                journal_states,
                            },
                        );
                    }
                    _ => unreachable!("state check guarantees Edited"),
                }
            }
            IssueAction::CancelUpload { id } => {
                let entry = self.entries.get(&id);
                if !matches!(entry, Some(IssueEntry::Uploading { .. })) {
                    panic!(
                        "cannot cancel issue upload while issue {id} is {}",
                        Self::entry_state_name(entry)
                    );
                }
                match self.entries.remove(&id).unwrap() {
                    IssueEntry::Uploading {
                        issue,
                        diffs,
                        journal_states,
                        ..
                    } => {
                        self.entries.insert(
                            id,
                            IssueEntry::Edited {
                                issue,
                                diffs,
                                failure: None,
                                journal_states,
                            },
                        );
                    }
                    _ => unreachable!("state check guarantees Uploading"),
                }
            }
            IssueAction::ClearUploadConflicts { id } => match self.entries.get_mut(&id) {
                Some(IssueEntry::Uploading { conflict, .. }) => *conflict = None,
                entry => panic!(
                    "cannot clear issue upload conflicts while issue {id} is {}",
                    Self::entry_state_name(entry.as_deref())
                ),
            },
            IssueAction::FailUpload { id, message } => {
                let entry = self.entries.get(&id);
                if !matches!(entry, Some(IssueEntry::Uploading { .. })) {
                    panic!(
                        "cannot fail issue upload while issue {id} is {}",
                        Self::entry_state_name(entry)
                    );
                }
                match self.entries.remove(&id).unwrap() {
                    IssueEntry::Uploading {
                        issue,
                        diffs,
                        journal_states,
                        ..
                    } => {
                        self.entries.insert(
                            id,
                            IssueEntry::Edited {
                                issue,
                                diffs,
                                failure: Some(message),
                                journal_states,
                            },
                        );
                    }
                    _ => unreachable!("state check guarantees Uploading"),
                }
            }
            IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
                children,
            } => {
                let id = server_issue.issue.id;
                if !matches!(self.entries.get(&id), Some(IssueEntry::Uploading { .. })) {
                    panic!(
                        "cannot retain issue upload conflicts while issue {id} is {}",
                        Self::entry_state_name(self.entries.get(&id))
                    );
                }
                self.assert_fetched_journals_are_valid(id, &server_issue.journals);
                let Some(IssueEntry::Uploading {
                    issue,
                    conflict,
                    journal_states,
                    ..
                }) = self.entries.get_mut(&id)
                else {
                    unreachable!("state check guarantees Uploading");
                };
                let current = std::mem::take(&mut issue.journals);
                issue.journals =
                    journal_states.merge_fetched(current, server_issue.journals.clone());
                *conflict = Some(IssueUploadConflict {
                    server_issue,
                    conflicts,
                });
                self.children.insert(id, children);
            }
            IssueAction::UploadSucceeded {
                mut issue,
                children,
            } => {
                let id = issue.issue.id;
                if !matches!(self.entries.get(&id), Some(IssueEntry::Uploading { .. })) {
                    panic!(
                        "cannot complete issue upload while issue {id} is {}",
                        Self::entry_state_name(self.entries.get(&id))
                    );
                }
                self.assert_fetched_journals_are_valid(id, &issue.journals);
                let Some(IssueEntry::Uploading {
                    issue: current,
                    mut journal_states,
                    ..
                }) = self.entries.remove(&id)
                else {
                    unreachable!("state check guarantees Uploading");
                };
                let fetched = std::mem::take(&mut issue.journals);
                issue.journals = journal_states.merge_fetched(current.journals, fetched);
                self.entries.insert(
                    id,
                    IssueEntry::Synced {
                        issue,
                        journal_states,
                    },
                );
                self.children.insert(id, children);
            }
            IssueAction::UpdateDescription { id, body } => self.update_issue(id, |issue| {
                let before = issue.description().to_string();
                IssuePropertyDiff::Description(IssueDescriptionDiff {
                    before,
                    after: body,
                })
            }),
            IssueAction::UpdateStatus { id, status_id } => self.update_issue(id, |issue| {
                let before = issue.status_id();
                IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                    before,
                    after: status_id,
                })
            }),
            IssueAction::UpdateTracker { id, tracker_id } => self.update_issue(id, |issue| {
                let before = issue.tracker_id();
                IssuePropertyDiff::TrackerId(IssueTrackerIdDiff {
                    before,
                    after: tracker_id,
                })
            }),
            IssueAction::UpdateProject { id, project_id } => {
                self.update_issue_with_diffs(id, |issue| {
                    let mut diffs = vec![IssuePropertyDiff::ProjectId(IssueProjectIdDiff {
                        before: issue.project_id(),
                        after: project_id,
                    })];
                    if let Some(before) = issue.target_version_id() {
                        diffs.push(IssuePropertyDiff::TargetVersionId(
                            IssueTargetVersionIdDiff {
                                before: Some(before),
                                after: None,
                            },
                        ));
                    }
                    if let Some(before) = issue.category_id() {
                        diffs.push(IssuePropertyDiff::CategoryId(IssueCategoryIdDiff {
                            before: Some(before),
                            after: None,
                        }));
                    }
                    diffs
                })
            }
            IssueAction::UpdatePriority { id, priority_id } => self.update_issue(id, |issue| {
                let before = issue.priority_id();
                IssuePropertyDiff::PriorityId(IssuePriorityIdDiff {
                    before,
                    after: priority_id,
                })
            }),
            IssueAction::UpdateAssignedTo { id, assigned_to_id } => {
                self.update_issue(id, |issue| {
                    let before = issue.assigned_to_id();
                    IssuePropertyDiff::AssignedToId(IssueAssignedToIdDiff {
                        before,
                        after: assigned_to_id,
                    })
                })
            }
            IssueAction::UpdateTargetVersion {
                id,
                target_version_id,
            } => self.update_issue(id, |issue| {
                let before = issue.target_version_id();
                IssuePropertyDiff::TargetVersionId(IssueTargetVersionIdDiff {
                    before,
                    after: target_version_id,
                })
            }),
            IssueAction::UpdateCategory { id, category_id } => self.update_issue(id, |issue| {
                let before = issue.category_id();
                IssuePropertyDiff::CategoryId(IssueCategoryIdDiff {
                    before,
                    after: category_id,
                })
            }),
            IssueAction::UpdateDoneRatio { id, done_ratio } => self.update_issue(id, |issue| {
                let before = issue.done_ratio();
                IssuePropertyDiff::DoneRatio(IssueDoneRatioDiff {
                    before,
                    after: done_ratio,
                })
            }),
            IssueAction::UpdateEstimatedHours {
                id,
                estimated_hours,
            } => self.update_issue(id, |issue| {
                let before = issue.estimated_hours();
                IssuePropertyDiff::EstimatedHours(IssueEstimatedHoursDiff {
                    before,
                    after: estimated_hours,
                })
            }),
            IssueAction::UpdateStartDate { id, start_date } => self.update_issue(id, |issue| {
                let before = issue.start_date();
                IssuePropertyDiff::StartDate(IssueStartDateDiff {
                    before,
                    after: start_date,
                })
            }),
            IssueAction::UpdateDueDate { id, due_date } => self.update_issue(id, |issue| {
                let before = issue.due_date();
                IssuePropertyDiff::DueDate(IssueDueDateDiff {
                    before,
                    after: due_date,
                })
            }),
        }
    }

    /// 初回取得の結果を、Issue本体とJournalが揃った状態で登録する。
    pub(super) fn complete_fetch(
        &mut self,
        id: IssueId,
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    ) {
        // 新しい同期結果やローカル編集を遅延した成功で上書きしないよう、
        // Fetching以外への着弾は制御破綻として拒否する。
        match self.entries.get(&id) {
            Some(IssueEntry::Fetching) => {}
            None => panic!("fetch succeeded for issue {id} without an issue state"),
            entry => panic!(
                "fetch succeeded while issue {id} is {}",
                Self::entry_state_name(entry)
            ),
        }
        let actual_id = issue.issue.id;
        if actual_id != id {
            panic!("fetch succeeded with mismatched issue id: requested {id}, got {actual_id}");
        }
        self.assert_fetched_journals_are_valid(id, &issue.journals);
        let journal_states = IssueJournalStates::synced(&issue.journals);
        self.entries.insert(
            id,
            IssueEntry::Synced {
                issue,
                journal_states,
            },
        );
        self.children.insert(id, children);
    }

    /// 詳細取得で得た子Issueの一覧を返す。子一覧を取得していないIssueでは空のsliceを返す。
    pub(super) fn get_issue_children(&self, issue_id: impl Into<IssueId>) -> &[IssueChild] {
        self.children
            .get(&issue_id.into())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn consume_journal_action(&mut self, action: JournalAction) {
        let issue_id = action.issue_id();
        let action = match action {
            JournalAction::CompleteRemoteUpload {
                journal_id,
                issue,
                children,
            } => {
                return self.take_in_journal_upload_fetch(issue, children, |journal_states| {
                    journal_states.complete_remote_upload(issue_id, journal_id)
                });
            }
            JournalAction::DetectRemoteUploadConflict {
                journal_id,
                issue,
                children,
            } => {
                let server_notes = issue
                    .journals
                    .iter()
                    .find(|journal| journal.id == journal_id)
                    .unwrap_or_else(|| {
                        panic!("fetched issue {issue_id} does not contain journal {journal_id}")
                    })
                    .notes
                    .clone();
                return self.take_in_journal_upload_fetch(issue, children, |journal_states| {
                    journal_states.detect_remote_upload_conflict(issue_id, journal_id, server_notes)
                });
            }
            JournalAction::EvacuateMissingRemoteUpload {
                journal_id,
                notes,
                issue,
                children,
            } => {
                if issue
                    .journals
                    .iter()
                    .any(|journal| journal.id == journal_id)
                {
                    panic!("fetched issue {issue_id} still contains journal {journal_id}");
                }
                return self.take_in_journal_upload_fetch(issue, children, |journal_states| {
                    journal_states.return_missing_remote_upload(issue_id, journal_id, notes)
                });
            }
            action => action,
        };
        match &action {
            JournalAction::CompleteLocalUploadWithFetched { journals, .. }
            | JournalAction::CompleteDeletedUploadWithFetched { journals, .. } => {
                self.assert_fetched_journals_are_valid(issue_id, journals);
            }
            JournalAction::StartLocalUpload { .. } => assert!(
                !matches!(
                    self.entries.get(&issue_id),
                    Some(IssueEntry::Uploading { .. })
                ),
                "cannot start local journal upload while issue {issue_id} is uploading"
            ),
            JournalAction::StartRemoteUpload { .. } => assert!(
                !matches!(
                    self.entries.get(&issue_id),
                    Some(IssueEntry::Uploading { .. })
                ),
                "cannot start remote journal upload while issue {issue_id} is uploading"
            ),
            JournalAction::StartDeletedUpload { .. } => assert!(
                !matches!(
                    self.entries.get(&issue_id),
                    Some(IssueEntry::Uploading { .. })
                ),
                "cannot start deleted journal upload while issue {issue_id} is uploading"
            ),
            _ => {}
        }
        let state_name = Self::entry_state_name(self.entries.get(&issue_id));
        let Some((issue, journal_states)) = self.loaded_parts_mut(issue_id) else {
            panic!("cannot update journals of issue {issue_id} while it is {state_name}");
        };
        journal_states.consume_action(&mut issue.journals, action);
    }

    /// Journal保存の取得結果を取り込む。`transition`で対象Journalの状態を遷移させてから、
    /// Issue本体・Journal・子一覧を取得値にする。
    ///
    /// Issue属性の差分は基準値と独立に保持するため、Editedのまま差分を残す。
    fn take_in_journal_upload_fetch(
        &mut self,
        mut fetched: IssueAggregate,
        children: Vec<IssueChild>,
        transition: impl FnOnce(&mut IssueJournalStates),
    ) {
        let id = fetched.issue.id;
        self.assert_fetched_journals_are_valid(id, &fetched.journals);
        let state_name = Self::entry_state_name(self.entries.get(&id));
        let (issue, journal_states) = match self.entries.get_mut(&id) {
            Some(IssueEntry::Synced {
                issue,
                journal_states,
            })
            | Some(IssueEntry::Edited {
                issue,
                journal_states,
                ..
            }) => (issue, journal_states),
            _ => panic!("cannot take in a journal upload result while issue {id} is {state_name}"),
        };
        transition(journal_states);
        let current = std::mem::take(&mut issue.journals);
        fetched.journals =
            journal_states.merge_fetched(current, std::mem::take(&mut fetched.journals));
        *issue = fetched;
        self.children.insert(id, children);
    }

    // Storeへ到達した取得結果のIDと所有関係の不整合は、取得失敗ではなくAction生成側の制御破綻として拒否する。
    fn assert_fetched_journals_are_valid(&self, issue_id: IssueId, journals: &[Journal]) {
        let mut seen: HashSet<JournalId> = HashSet::with_capacity(journals.len());
        for journal in journals {
            let journal_id = journal.id;
            if !seen.insert(journal_id) {
                panic!(
                    "fetched journals for issue {issue_id} contain duplicate journal {journal_id}"
                );
            }
            if journal.issue_id != issue_id {
                panic!(
                    "fetched journal {journal_id} of issue {issue_id} has issue {}",
                    journal.issue_id
                );
            }
        }
        for (other_issue_id, entry) in &self.entries {
            if *other_issue_id == issue_id {
                continue;
            }
            let Some((other_issue, _)) = Self::loaded_parts(Some(entry)) else {
                continue;
            };
            if let Some(journal) = other_issue
                .journals
                .iter()
                .find(|registered| seen.contains(&registered.id))
            {
                panic!(
                    "remote journal {} is already registered for issue {other_issue_id}",
                    journal.id
                );
            }
        }
    }

    fn loaded_parts(entry: Option<&IssueEntry>) -> Option<(&IssueAggregate, &IssueJournalStates)> {
        match entry? {
            IssueEntry::Synced {
                issue,
                journal_states,
            }
            | IssueEntry::Edited {
                issue,
                journal_states,
                ..
            }
            | IssueEntry::Uploading {
                issue,
                journal_states,
                ..
            } => Some((issue, journal_states)),
            IssueEntry::Fetching | IssueEntry::FetchFailed { .. } => None,
        }
    }

    fn loaded_parts_mut(
        &mut self,
        issue_id: IssueId,
    ) -> Option<(&mut IssueAggregate, &mut IssueJournalStates)> {
        match self.entries.get_mut(&issue_id)? {
            IssueEntry::Synced {
                issue,
                journal_states,
            }
            | IssueEntry::Edited {
                issue,
                journal_states,
                ..
            }
            | IssueEntry::Uploading {
                issue,
                journal_states,
                ..
            } => Some((issue, journal_states)),
            IssueEntry::Fetching | IssueEntry::FetchFailed { .. } => None,
        }
    }

    #[track_caller]
    pub(super) fn get_issue(&self, issue_id: impl Into<IssueId>) -> (IssueView<'_>, IssueState) {
        let issue_id = issue_id.into();
        let entry = self.entries.get(&issue_id);
        match Self::loaded_view(entry) {
            Some(loaded) => loaded,
            None => panic!(
                "cannot get issue {issue_id} while it is {}",
                Self::entry_state_name(entry)
            ),
        }
    }

    pub(super) fn get_issues(&self) -> impl Iterator<Item = (IssueView<'_>, IssueState)> {
        self.entries
            .values()
            .filter_map(|entry| Self::loaded_view(Some(entry)))
    }

    fn loaded_view(entry: Option<&IssueEntry>) -> Option<(IssueView<'_>, IssueState)> {
        match entry? {
            IssueEntry::Synced { issue, .. } => {
                Some((IssueView::new(issue, &[]), IssueState::Synced))
            }
            IssueEntry::Edited { issue, diffs, .. } => {
                Some((IssueView::new(issue, diffs), IssueState::Edited))
            }
            IssueEntry::Uploading { issue, diffs, .. } => {
                Some((IssueView::new(issue, diffs), IssueState::Uploading))
            }
            IssueEntry::Fetching | IssueEntry::FetchFailed { .. } => None,
        }
    }

    pub(super) fn get_issue_property_diffs(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> &[IssuePropertyDiff] {
        match self.entries.get(&issue_id.into()) {
            Some(IssueEntry::Edited { diffs, .. }) => &diffs[..],
            Some(IssueEntry::Uploading { diffs, .. }) => &diffs[..],
            _ => &[],
        }
    }

    pub(super) fn try_get_issue_upload_conflict(
        &self,
        id: IssueId,
    ) -> Option<(&IssueAggregate, &[IssuePropertyDiff])> {
        match self.entries.get(&id) {
            Some(IssueEntry::Uploading {
                conflict: Some(conflict),
                ..
            }) => Some((&conflict.server_issue, &conflict.conflicts[..])),
            _ => None,
        }
    }

    pub(super) fn try_get_issue_upload_failure(&self, id: IssueId) -> Option<&str> {
        match self.entries.get(&id) {
            Some(IssueEntry::Edited {
                failure: Some(failure),
                ..
            }) => Some(failure.as_str()),
            _ => None,
        }
    }

    pub(super) fn try_get_issue_state(&self, issue_id: impl Into<IssueId>) -> Option<IssueState> {
        match self.entries.get(&issue_id.into()) {
            Some(IssueEntry::Synced { .. }) => Some(IssueState::Synced),
            Some(IssueEntry::Edited { .. }) => Some(IssueState::Edited),
            Some(IssueEntry::Uploading { .. }) => Some(IssueState::Uploading),
            _ => None,
        }
    }

    pub(super) fn try_get_issue_fetch_state(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<IssueFetchState> {
        match self.entries.get(&issue_id.into()) {
            Some(IssueEntry::Fetching) => Some(IssueFetchState::Fetching),
            Some(IssueEntry::FetchFailed { message }) => Some(IssueFetchState::FetchFailed {
                message: message.clone(),
            }),
            _ => None,
        }
    }

    /// Issueが取得済みでない場合は空のVecを返す。
    pub(super) fn get_remote_journals(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Vec<RemoteJournalView<'_>> {
        let issue_id = issue_id.into();
        let Some((issue, journal_states)) = Self::loaded_parts(self.entries.get(&issue_id)) else {
            return Vec::new();
        };
        issue
            .journals
            .iter()
            .map(|journal| RemoteJournalView {
                journal,
                state: journal_states
                    .remote_state(journal.id)
                    .expect("every journal of a loaded issue has a state"),
            })
            .collect()
    }

    #[track_caller]
    pub(super) fn get_remote_journal(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> RemoteJournalView<'_> {
        let issue_id = issue_id.into();
        let journal_id = journal_id.into();
        match self
            .get_remote_journals(issue_id)
            .into_iter()
            .find(|entry| entry.journal.id == journal_id)
        {
            Some(entry) => entry,
            None => panic!("remote journal {journal_id} is not registered for issue {issue_id}"),
        }
    }

    pub(super) fn try_get_local_journal(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<&LocalJournalEntry> {
        Self::loaded_parts(self.entries.get(&issue_id.into()))
            .and_then(|(_, journal_states)| journal_states.local())
    }

    /// Issueが取得済みでない場合は空のsliceを返す。
    pub(super) fn get_deleted_journals(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> &[DeletedJournalEntry] {
        Self::loaded_parts(self.entries.get(&issue_id.into()))
            .map(|(_, journal_states)| journal_states.deleted())
            .unwrap_or(&[])
    }

    pub(super) fn has_uploading_journal(&self, issue_id: impl Into<IssueId>) -> bool {
        Self::loaded_parts(self.entries.get(&issue_id.into()))
            .is_some_and(|(_, journal_states)| journal_states.has_uploading())
    }

    pub(super) fn try_get_remote_journal_upload_conflict(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> Option<(&JournalNotesDiff, &RemoteJournalUploadConflict)> {
        let (_, journal_states) = Self::loaded_parts(self.entries.get(&issue_id.into()))?;
        match journal_states.remote_state(journal_id.into())? {
            RemoteJournalState::Uploading {
                diff,
                conflict: Some(conflict),
            } => Some((diff, conflict)),
            _ => None,
        }
    }

    fn entry_state_name(entry: Option<&IssueEntry>) -> &'static str {
        match entry {
            None => "Unregistered",
            Some(IssueEntry::Fetching) => "Fetching",
            Some(IssueEntry::FetchFailed { .. }) => "FetchFailed",
            Some(IssueEntry::Synced { .. }) => "Synced",
            Some(IssueEntry::Edited { .. }) => "Edited",
            Some(IssueEntry::Uploading { .. }) => "Uploading",
        }
    }

    fn update_issue(&mut self, id: IssueId, edit: impl FnOnce(IssueView<'_>) -> IssuePropertyDiff) {
        self.update_issue_with_diffs(id, |issue| vec![edit(issue)]);
    }

    /// 1つの操作で複数のpropertyを変更する場合に、変更したproperty分のdiffをまとめて記録する。
    ///
    /// `edit`は編集を反映した現在の値を受け取り、その値を`before`にしたdiffを返す。
    fn update_issue_with_diffs(
        &mut self,
        id: IssueId,
        edit: impl FnOnce(IssueView<'_>) -> Vec<IssuePropertyDiff>,
    ) {
        match self.entries.get(&id) {
            Some(IssueEntry::Synced { .. }) | Some(IssueEntry::Edited { .. }) => {}
            Some(IssueEntry::Uploading { .. }) => {
                panic!("cannot update issue while issue {id} is Uploading");
            }
            Some(IssueEntry::Fetching) => {
                panic!("cannot update issue while issue {id} is Fetching")
            }
            Some(IssueEntry::FetchFailed { .. }) => {
                panic!("cannot update issue while issue {id} is FetchFailed")
            }
            None => panic!("cannot update missing issue {id}"),
        }
        let (issue, mut diffs, failure, journal_states) = match self.entries.remove(&id).unwrap() {
            IssueEntry::Synced {
                issue,
                journal_states,
            } => (issue, Vec::new(), None, journal_states),
            IssueEntry::Edited {
                issue,
                diffs,
                failure,
                journal_states,
            } => (issue, diffs, failure, journal_states),
            _ => {
                unreachable!("update_issue受理検査により、遷移先はSynced / Edited以外が存在しない")
            }
        };
        let edited = edit(IssueView::new(&issue, &diffs));
        diffs.extend(edited);
        // diffは編集履歴として順に残し、差し引きで変更がなくなった時点でSyncedへ戻す。
        let entry = if fold_property_diffs(&diffs).is_empty() {
            IssueEntry::Synced {
                issue,
                journal_states,
            }
        } else {
            IssueEntry::Edited {
                issue,
                diffs,
                failure,
                journal_states,
            }
        };
        self.entries.insert(id, entry);
    }
}
