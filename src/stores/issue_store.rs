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
    issues: HashMap<IssueId, IssueAggregate>,
    issue_states: HashMap<IssueId, IssueState>,
    issue_property_diffs: HashMap<IssueId, Vec<IssuePropertyDiff>>,
    issue_upload_conflicts: HashMap<IssueId, (IssueAggregate, Vec<IssuePropertyDiff>)>,
    issue_upload_failures: HashMap<IssueId, String>,
}

impl IssueStore {
    pub(super) fn new() -> Self {
        Self {
            issues: HashMap::new(),
            issue_states: HashMap::new(),
            issue_property_diffs: HashMap::new(),
            issue_upload_conflicts: HashMap::new(),
            issue_upload_failures: HashMap::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: IssueAction) {
        match action {
            IssueAction::Load { id } => {
                if !self.issues.contains_key(&id) && !self.issue_states.contains_key(&id) {
                    self.issues.insert(id, parse_issue_yaml(id.get()));
                    self.issue_states.insert(id, IssueState::Synced);
                    self.issue_property_diffs.entry(id).or_default();
                }
            }
            IssueAction::Sync { issue } => {
                let id = issue.issue.id;
                if let Some(state) = self.get_issue_state(id)
                    && !matches!(state, IssueState::Edited | IssueState::Uploading)
                {
                    panic!("cannot sync issue {id} while it is {state:?}");
                }
                if self.issues.contains_key(&id) && !self.issue_states.contains_key(&id) {
                    panic!("cannot sync issue {id} without an issue state");
                }
                self.issues.insert(id, issue);
                self.issue_property_diffs.remove(&id);
                self.issue_upload_conflicts.remove(&id);
                self.issue_upload_failures.remove(&id);
                self.issue_states.insert(id, IssueState::Synced);
            }
            IssueAction::StartFetching { id } => {
                // UIとfetch usecaseはstateがNoneまたはFetchFailedの場合にだけこのActionを発行する。
                // Fetchingや取得済み状態への着弾は呼び出し側の不変条件違反として拒否する。
                let state = self.get_issue_state(id);
                let can_start = match state {
                    None => !self.issues.contains_key(&id),
                    Some(IssueState::FetchFailed { .. }) => true,
                    Some(_) => false,
                };
                if !can_start {
                    panic!("cannot start fetching issue {id} while it is {state:?}");
                }
                self.issues.remove(&id);
                self.issue_property_diffs.remove(&id);
                self.issue_upload_conflicts.remove(&id);
                self.issue_upload_failures.remove(&id);
                self.issue_states.insert(id, IssueState::Fetching);
            }
            IssueAction::FetchSucceeded { id, issue } => {
                // 新しい同期結果やローカル編集を遅延した成功で上書きしないよう、
                // Fetching以外への着弾は制御破綻として拒否する。
                match self.get_issue_state(id) {
                    Some(IssueState::Fetching) => {}
                    Some(state) => panic!("fetch succeeded while issue {id} is {state:?}"),
                    None => panic!("fetch succeeded for issue {id} without an issue state"),
                }
                let actual_id = issue.issue.id;
                if actual_id != id {
                    panic!(
                        "fetch succeeded with mismatched issue id: requested {id}, got {actual_id}"
                    );
                }
                self.issues.insert(id, issue);
                self.issue_property_diffs.remove(&id);
                self.issue_upload_conflicts.remove(&id);
                self.issue_upload_failures.remove(&id);
                self.issue_states.insert(id, IssueState::Synced);
            }
            IssueAction::FetchFailed { id, message } => {
                // 新しい同期結果やローカル編集を遅延した失敗で破棄しないよう、
                // Fetching以外への着弾は制御破綻として拒否する。
                match self.get_issue_state(id) {
                    Some(IssueState::Fetching) => {}
                    Some(state) => panic!("fetch failed while issue {id} is {state:?}"),
                    None => panic!("fetch failed for issue {id} without an issue state"),
                }
                self.issues.remove(&id);
                self.issue_property_diffs.remove(&id);
                self.issue_upload_conflicts.remove(&id);
                self.issue_upload_failures.remove(&id);
                self.issue_states
                    .insert(id, IssueState::FetchFailed { message });
            }
            IssueAction::StartUpload { id } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Edited {
                    panic!("cannot start issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
                self.issue_upload_failures.remove(&id);
                self.issue_states.insert(id, IssueState::Uploading);
            }
            IssueAction::CancelUpload { id } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Uploading {
                    panic!("cannot cancel issue upload while issue {id} is {state:?}");
                }
                // Uploadingへ入るStartUploadが以前のfailureを破棄済みなので、ここではclear不要。
                self.issue_upload_conflicts.remove(&id);
                self.issue_states.insert(id, IssueState::Edited);
            }
            IssueAction::ClearUploadConflicts { id } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Uploading {
                    panic!("cannot clear issue upload conflicts while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
            }
            IssueAction::FailUpload { id, message } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Uploading {
                    panic!("cannot fail issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
                self.issue_upload_failures.insert(id, message);
                self.issue_states.insert(id, IssueState::Edited);
            }
            IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
            } => {
                let id = server_issue.issue.id;
                let state = self.state_or_synced(id);
                if state != &IssueState::Uploading {
                    panic!("cannot retain issue upload conflicts while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts
                    .insert(id, (server_issue, conflicts));
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
            IssueAction::UpdateCategory { id, category_id } => {
                let issue = self.assert_can_update_issue(id);
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
            IssueAction::UpdateDoneRatio { id, done_ratio } => {
                let issue = self.assert_can_update_issue(id);
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
            IssueAction::UpdateStartDate { id, start_date } => {
                let issue = self.assert_can_update_issue(id);
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
            IssueAction::UpdateDueDate { id, due_date } => {
                let issue = self.assert_can_update_issue(id);
                let before = issue.due_date;
                issue.due_date = due_date;
                self.issue_property_diffs
                    .entry(id)
                    .or_default()
                    .push(IssuePropertyDiff::DueDate(IssueDueDateDiff {
                        before,
                        after: due_date,
                    }));
                self.issue_states.insert(id, IssueState::Edited);
            }
        }
    }

    pub(super) fn get_issue(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> Option<(&IssueAggregate, &IssueState)> {
        let issue_id = issue_id.into();
        self.issues
            .get(&issue_id)
            .zip(self.get_issue_state(issue_id))
    }

    pub(super) fn get_issues(&self) -> &HashMap<IssueId, IssueAggregate> {
        &self.issues
    }

    pub(super) fn get_issue_property_diffs(
        &self,
        issue_id: impl Into<IssueId>,
    ) -> &[IssuePropertyDiff] {
        self.issue_property_diffs
            .get(&issue_id.into())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn get_issue_upload_conflict(
        &self,
        id: IssueId,
    ) -> Option<(&IssueAggregate, &[IssuePropertyDiff])> {
        self.issue_upload_conflicts
            .get(&id)
            .map(|(issue, conflicts)| (issue, conflicts.as_slice()))
    }

    pub(super) fn get_issue_upload_failure(&self, id: IssueId) -> Option<&str> {
        self.issue_upload_failures.get(&id).map(String::as_str)
    }

    pub(super) fn get_issue_state(&self, issue_id: impl Into<IssueId>) -> Option<&IssueState> {
        self.issue_states.get(&issue_id.into())
    }

    fn state_or_synced(&self, issue_id: impl Into<IssueId>) -> &IssueState {
        self.get_issue_state(issue_id)
            .unwrap_or(&IssueState::Synced)
    }

    fn update_issue(
        &mut self,
        id: IssueId,
        edit: impl FnOnce(&mut IssueAggregate) -> IssuePropertyDiff,
    ) {
        let issue = self.assert_can_update_issue(id);
        let diff = edit(issue);
        self.issue_property_diffs.entry(id).or_default().push(diff);
        self.issue_states.insert(id, IssueState::Edited);
    }

    fn assert_can_update_issue(&mut self, id: IssueId) -> &mut IssueAggregate {
        // stateが未登録ならSyncedとみなすため、先にentityの欠損を状態異常として拒否する。
        let issue = self
            .issues
            .get_mut(&id)
            .unwrap_or_else(|| panic!("cannot update missing issue {id}"));
        let state = self.issue_states.get(&id).unwrap_or(&IssueState::Synced);
        if state == &IssueState::Uploading {
            panic!("cannot update issue while issue {id} is {state:?}");
        }
        issue
    }
}
