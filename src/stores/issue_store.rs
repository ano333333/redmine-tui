//! Issueの取得・編集・uploadに伴う状態を保持し、Actionで遷移させる。
//! entryのvariantで本体・差分・失敗・競合の保持可能な組合せを制限する。
//! Actionごとの遷移前提条件は受理時に検査する。

use std::collections::HashMap;

use crate::entities::IssueAggregate;
use crate::libs::yaml::parse_issue_yaml;
use crate::vos::issue_property_diff::{
    IssueAssignedToIdDiff, IssueCategoryIdDiff, IssueDescriptionDiff, IssueDoneRatioDiff,
    IssueDueDateDiff, IssueStartDateDiff, IssueStatusIdDiff, IssueTargetVersionIdDiff,
};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssuePropertyDiff, IssueStatusId, TargetVersionId, UserId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueState {
    Fetching,
    FetchFailed { message: String },
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
    },
    Edited {
        issue: IssueAggregate,
        diffs: Vec<IssuePropertyDiff>,
        failure: Option<String>,
    },
    Uploading {
        issue: IssueAggregate,
        diffs: Vec<IssuePropertyDiff>,
        conflict: Option<IssueUploadConflict>,
    },
}

#[derive(Debug)]
struct IssueUploadConflict {
    server_issue: IssueAggregate,
    conflicts: Vec<IssuePropertyDiff>,
}

pub enum IssueAction {
    Load {
        id: IssueId,
    },
    Sync {
        issue: IssueAggregate,
    },
    StartFetching {
        id: IssueId,
    },
    FetchSucceeded {
        id: IssueId,
        issue: IssueAggregate,
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
    UploadConflictsDetected {
        server_issue: IssueAggregate,
        conflicts: Vec<IssuePropertyDiff>,
    },
    UpdateDescription {
        id: IssueId,
        body: String,
    },
    UpdateStatus {
        id: IssueId,
        status_id: IssueStatusId,
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
}

impl IssueStore {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: IssueAction) {
        match action {
            IssueAction::Load { id } => {
                if !self.entries.contains_key(&id) {
                    self.entries.insert(
                        id,
                        IssueEntry::Synced {
                            issue: parse_issue_yaml(id.get()),
                        },
                    );
                }
            }
            IssueAction::Sync { issue } => {
                let id = issue.issue.id;
                let state = self.derived_state(id);
                if let Some(state) = state {
                    if !matches!(state, IssueState::Edited | IssueState::Uploading) {
                        panic!("cannot sync issue {id} while it is {state:?}");
                    }
                }
                self.entries.insert(id, IssueEntry::Synced { issue });
            }
            IssueAction::StartFetching { id } => {
                // UIとfetch usecaseはstateがNoneまたはFetchFailedの場合にだけこのActionを発行する。
                // Fetchingや取得済み状態への着弾は呼び出し側の不変条件違反として拒否する。
                let state = self.derived_state(id);
                if !matches!(state, None | Some(IssueState::FetchFailed { .. })) {
                    panic!("cannot start fetching issue {id} while it is {state:?}");
                }
                self.entries.insert(id, IssueEntry::Fetching);
            }
            IssueAction::FetchSucceeded { id, issue } => {
                // 新しい同期結果やローカル編集を遅延した成功で上書きしないよう、
                // Fetching以外への着弾は制御破綻として拒否する。
                let state = self.derived_state(id);
                if !matches!(state, Some(IssueState::Fetching)) {
                    match state {
                        None => panic!("fetch succeeded for issue {id} without an issue state"),
                        Some(state) => panic!("fetch succeeded while issue {id} is {state:?}"),
                    }
                }
                let actual_id = issue.issue.id;
                if actual_id != id {
                    panic!(
                        "fetch succeeded with mismatched issue id: requested {id}, got {actual_id}"
                    );
                }
                self.entries.insert(id, IssueEntry::Synced { issue });
            }
            IssueAction::FetchFailed { id, message } => {
                // 新しい同期結果やローカル編集を遅延した失敗で破棄しないよう、
                // Fetching以外への着弾は制御破綻として拒否する。
                let state = self.derived_state(id);
                if !matches!(state, Some(IssueState::Fetching)) {
                    match state {
                        None => panic!("fetch failed for issue {id} without an issue state"),
                        Some(state) => panic!("fetch failed while issue {id} is {state:?}"),
                    }
                }
                self.entries.insert(id, IssueEntry::FetchFailed { message });
            }
            IssueAction::StartUpload { id } => {
                let state = self.derived_state(id).unwrap_or(IssueState::Synced);
                if !matches!(state, IssueState::Edited) {
                    panic!("cannot start issue upload while issue {id} is {state:?}");
                }
                match self.entries.remove(&id).unwrap() {
                    IssueEntry::Edited { issue, diffs, .. } => {
                        self.entries.insert(
                            id,
                            IssueEntry::Uploading {
                                issue,
                                diffs,
                                conflict: None,
                            },
                        );
                    }
                    _ => unreachable!("state check guarantees Edited"),
                }
            }
            IssueAction::CancelUpload { id } => {
                let state = self.derived_state(id).unwrap_or(IssueState::Synced);
                if !matches!(state, IssueState::Uploading) {
                    panic!("cannot cancel issue upload while issue {id} is {state:?}");
                }
                match self.entries.remove(&id).unwrap() {
                    IssueEntry::Uploading { issue, diffs, .. } => {
                        self.entries.insert(
                            id,
                            IssueEntry::Edited {
                                issue,
                                diffs,
                                failure: None,
                            },
                        );
                    }
                    _ => unreachable!("state check guarantees Uploading"),
                }
            }
            IssueAction::ClearUploadConflicts { id } => match self.entries.get_mut(&id) {
                Some(IssueEntry::Uploading { conflict, .. }) => *conflict = None,
                Some(entry) => panic!(
                    "cannot clear issue upload conflicts while issue {id} is {:?}",
                    Self::entry_state(entry)
                ),
                None => panic!(
                    "cannot clear issue upload conflicts while issue {id} is {:?}",
                    IssueState::Synced
                ),
            },
            IssueAction::FailUpload { id, message } => {
                let state = self.derived_state(id).unwrap_or(IssueState::Synced);
                if !matches!(state, IssueState::Uploading) {
                    panic!("cannot fail issue upload while issue {id} is {state:?}");
                }
                match self.entries.remove(&id).unwrap() {
                    IssueEntry::Uploading { issue, diffs, .. } => {
                        self.entries.insert(
                            id,
                            IssueEntry::Edited {
                                issue,
                                diffs,
                                failure: Some(message),
                            },
                        );
                    }
                    _ => unreachable!("state check guarantees Uploading"),
                }
            }
            IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
            } => {
                let id = server_issue.issue.id;
                match self.entries.get_mut(&id) {
                    Some(IssueEntry::Uploading { conflict, .. }) => {
                        *conflict = Some(IssueUploadConflict {
                            server_issue,
                            conflicts,
                        });
                    }
                    Some(entry) => panic!(
                        "cannot retain issue upload conflicts while issue {id} is {:?}",
                        Self::entry_state(entry)
                    ),
                    None => panic!(
                        "cannot retain issue upload conflicts while issue {id} is {:?}",
                        IssueState::Synced
                    ),
                }
            }
            IssueAction::UpdateDescription { id, body } => self.update_issue(id, |issue| {
                let before = issue.issue.description.clone();
                issue.issue.description = body.clone();
                IssuePropertyDiff::Description(IssueDescriptionDiff {
                    before,
                    after: body,
                })
            }),
            IssueAction::UpdateStatus { id, status_id } => self.update_issue(id, |issue| {
                let before = issue.issue.status_id;
                issue.issue.status_id = status_id;
                IssuePropertyDiff::StatusId(IssueStatusIdDiff {
                    before,
                    after: status_id,
                })
            }),
            IssueAction::UpdateAssignedTo { id, assigned_to_id } => {
                self.update_issue(id, |issue| {
                    let before = issue.assigned_to_id;
                    issue.assigned_to_id = assigned_to_id;
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
                let before = issue.target_version_id;
                issue.target_version_id = target_version_id;
                IssuePropertyDiff::TargetVersionId(IssueTargetVersionIdDiff {
                    before,
                    after: target_version_id,
                })
            }),
            IssueAction::UpdateCategory { id, category_id } => self.update_issue(id, |issue| {
                let before = issue.category_id;
                issue.category_id = category_id;
                IssuePropertyDiff::CategoryId(IssueCategoryIdDiff {
                    before,
                    after: category_id,
                })
            }),
            IssueAction::UpdateDoneRatio { id, done_ratio } => self.update_issue(id, |issue| {
                let before = issue.done_ratio;
                issue.done_ratio = done_ratio;
                IssuePropertyDiff::DoneRatio(IssueDoneRatioDiff {
                    before,
                    after: done_ratio,
                })
            }),
            IssueAction::UpdateStartDate { id, start_date } => self.update_issue(id, |issue| {
                let before = issue.start_date;
                issue.start_date = start_date;
                IssuePropertyDiff::StartDate(IssueStartDateDiff {
                    before,
                    after: start_date,
                })
            }),
            IssueAction::UpdateDueDate { id, due_date } => self.update_issue(id, |issue| {
                let before = issue.due_date;
                issue.due_date = due_date;
                IssuePropertyDiff::DueDate(IssueDueDateDiff {
                    before,
                    after: due_date,
                })
            }),
        }
    }

    pub(super) fn get_issue(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<(&IssueAggregate, IssueState)> {
        let issue_id = issue_id.into();
        match self.entries.get(&issue_id) {
            Some(IssueEntry::Synced { issue }) => return Some((issue, IssueState::Synced)),
            Some(IssueEntry::Edited { issue, .. }) => return Some((issue, IssueState::Edited)),
            Some(IssueEntry::Uploading { issue, .. }) => {
                return Some((issue, IssueState::Uploading));
            }
            _ => {}
        }
        None
    }

    pub(super) fn get_issues(&self) -> impl Iterator<Item = (&IssueId, &IssueAggregate)> {
        self.entries.iter().filter_map(|(id, entry)| {
            if let IssueEntry::Synced { issue }
            | IssueEntry::Edited { issue, .. }
            | IssueEntry::Uploading { issue, .. } = entry
            {
                Some((id, issue))
            } else {
                None
            }
        })
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

    pub(super) fn get_issue_upload_conflict(
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

    pub(super) fn get_issue_upload_failure(&self, id: IssueId) -> Option<&str> {
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

    fn derived_state(&self, id: IssueId) -> Option<IssueState> {
        self.entries.get(&id).map(Self::entry_state)
    }

    fn entry_state(entry: &IssueEntry) -> IssueState {
        match entry {
            IssueEntry::Fetching => IssueState::Fetching,
            IssueEntry::FetchFailed { message } => IssueState::FetchFailed {
                message: message.clone(),
            },
            IssueEntry::Synced { .. } => IssueState::Synced,
            IssueEntry::Edited { .. } => IssueState::Edited,
            IssueEntry::Uploading { .. } => IssueState::Uploading,
        }
    }

    fn update_issue(
        &mut self,
        id: IssueId,
        edit: impl FnOnce(&mut IssueAggregate) -> IssuePropertyDiff,
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
        let entry = self.entries.remove(&id).unwrap();
        match entry {
            IssueEntry::Synced { mut issue } => {
                let diff = edit(&mut issue);
                self.entries.insert(
                    id,
                    IssueEntry::Edited {
                        issue,
                        diffs: vec![diff],
                        failure: None,
                    },
                );
            }
            IssueEntry::Edited {
                mut issue,
                mut diffs,
                failure,
            } => {
                let diff = edit(&mut issue);
                diffs.push(diff);
                self.entries.insert(
                    id,
                    IssueEntry::Edited {
                        issue,
                        diffs,
                        failure,
                    },
                );
            }
            _ => {
                unreachable!("update_issue受理検査により、遷移先はSynced / Edited以外が存在しない")
            }
        }
    }
}
