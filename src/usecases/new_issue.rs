use std::path::PathBuf;

use chrono::{DateTime, Local};
use uuid::Uuid;

use crate::vos::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

/// 新規作成用Componentごとに割り振り、作成の要求とその結果のイベントを対応付ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssueCreationId(Uuid);

impl IssueCreationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// 作成前のIssueの下書き。新規作成用Componentが持つ。
///
/// `project_id`と`parent_issue_id`は作成を始めた導線で決まる。
/// トラッカー・ステータス・優先度の`None`は、Redmineの既定値にすることを表す。
#[derive(Clone, Debug, PartialEq)]
pub struct NewIssueDraft {
    pub project_id: ProjectId,
    pub parent_issue_id: Option<IssueId>,
    pub subject: String,
    pub description: String,
    pub tracker_id: Option<TrackerId>,
    pub status_id: Option<IssueStatusId>,
    pub priority_id: Option<PriorityId>,
    pub assigned_to_id: Option<UserId>,
    pub target_version_id: Option<TargetVersionId>,
    pub start_date: Option<DateTime<Local>>,
    pub due_date: Option<DateTime<Local>>,
    pub done_ratio: Option<u16>,
    pub estimated_hours: Option<f64>,
    pub category_id: Option<CategoryId>,
    pub attachments: Vec<DraftAttachment>,
}

/// 作成と同時に添付するファイル。ファイルは作成を要求したときに読む。
#[derive(Clone, Debug, PartialEq)]
pub struct DraftAttachment {
    pub filename: String,
    pub path: PathBuf,
}

impl NewIssueDraft {
    pub fn new(project_id: ProjectId, parent_issue_id: Option<IssueId>) -> Self {
        Self {
            project_id,
            parent_issue_id,
            subject: String::new(),
            description: String::new(),
            tracker_id: None,
            status_id: None,
            priority_id: None,
            assigned_to_id: None,
            target_version_id: None,
            start_date: None,
            due_date: None,
            done_ratio: None,
            estimated_hours: None,
            category_id: None,
            attachments: Vec::new(),
        }
    }
}
