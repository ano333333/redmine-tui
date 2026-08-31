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
}

impl IssueStore {
    pub(super) fn new() -> Self {
        Self {
            issues: HashMap::new(),
            issue_states: HashMap::new(),
            issue_property_diffs: HashMap::new(),
            issue_upload_conflicts: HashMap::new(),
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
                self.issue_states.insert(id, IssueState::Synced);
            }
            IssueAction::StartFetching { id } => {
                let can_start = match self.get_issue_state(id) {
                    None => !self.issues.contains_key(&id),
                    Some(IssueState::FetchFailed { .. }) => true,
                    Some(_) => false,
                };
                if can_start {
                    self.issues.remove(&id);
                    self.issue_property_diffs.remove(&id);
                    self.issue_upload_conflicts.remove(&id);
                    self.issue_states.insert(id, IssueState::Fetching);
                }
            }
            IssueAction::FetchSucceeded { id, issue } => {
                if matches!(self.get_issue_state(id), Some(IssueState::Fetching))
                    && issue.issue.id == id
                {
                    self.issues.insert(id, issue);
                    self.issue_property_diffs.remove(&id);
                    self.issue_upload_conflicts.remove(&id);
                    self.issue_states.insert(id, IssueState::Synced);
                }
            }
            IssueAction::FetchFailed { id, message } => {
                if matches!(self.get_issue_state(id), Some(IssueState::Fetching)) {
                    self.issues.remove(&id);
                    self.issue_property_diffs.remove(&id);
                    self.issue_upload_conflicts.remove(&id);
                    self.issue_states
                        .insert(id, IssueState::FetchFailed { message });
                }
            }
            IssueAction::StartUpload { id } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Edited {
                    panic!("cannot start issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
                self.issue_states.insert(id, IssueState::Uploading);
            }
            IssueAction::CancelUpload { id } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Uploading {
                    panic!("cannot cancel issue upload while issue {id} is {state:?}");
                }
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
            IssueAction::FailUpload { id } => {
                let state = self.state_or_synced(id);
                if state != &IssueState::Uploading {
                    panic!("cannot fail issue upload while issue {id} is {state:?}");
                }
                self.issue_upload_conflicts.remove(&id);
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
            IssueAction::UpdateDescription { id, body } => {
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
            IssueAction::UpdateStatus { id, status_id } => {
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
            IssueAction::UpdateAssignedTo { id, assigned_to_id } => {
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
            IssueAction::UpdateTargetVersion {
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
            IssueAction::UpdateCategory { id, category_id } => {
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
            IssueAction::UpdateDoneRatio { id, done_ratio } => {
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
            IssueAction::UpdateStartDate { id, start_date } => {
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
            IssueAction::UpdateDueDate { id, due_date } => {
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

    pub(super) fn get_issue_state(&self, issue_id: impl Into<IssueId>) -> Option<&IssueState> {
        self.issue_states.get(&issue_id.into())
    }

    fn state_or_synced(&self, issue_id: impl Into<IssueId>) -> &IssueState {
        self.get_issue_state(issue_id)
            .unwrap_or(&IssueState::Synced)
    }

    fn can_update_issue(&self, id: impl Into<IssueId>) -> bool {
        let id = id.into();
        let state = self.state_or_synced(id);
        if state == &IssueState::Uploading {
            panic!("cannot update issue while issue {id} is {state:?}");
        }
        true
    }
}
