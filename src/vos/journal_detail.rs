use chrono::{DateTime, Local};

use crate::vos::id::{
    CategoryId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

#[derive(Clone)]
pub enum JournalDetailAttr {
    StatusId {
        old: IssueStatusId,
        new: IssueStatusId,
    },
    TrackerId {
        old: TrackerId,
        new: TrackerId,
    },
    ProjectId {
        old: ProjectId,
        new: ProjectId,
    },
    Subject {
        old: String,
        new: String,
    },
    Description {
        old: String,
        new: String,
    },
    CategoryId {
        old: Option<CategoryId>,
        new: Option<CategoryId>,
    },
    AssignedToId {
        old: Option<UserId>,
        new: Option<UserId>,
    },
    PriorityId {
        old: PriorityId,
        new: PriorityId,
    },
    // Redmineの属性名はfixed_version_idだが、対応するIssueのフィールド・型はtarget_version_id/TargetVersionId
    FixedVersionId {
        old: Option<TargetVersionId>,
        new: Option<TargetVersionId>,
    },
    AuthorId {
        old: UserId,
        new: UserId,
    },
    StartDate {
        old: Option<DateTime<Local>>,
        new: Option<DateTime<Local>>,
    },
    DueDate {
        old: Option<DateTime<Local>>,
        new: Option<DateTime<Local>>,
    },
    DoneRatio {
        old: u16,
        new: u16,
    },
    EstimatedHours {
        old: Option<u16>,
        new: Option<u16>,
    },
    ParentId {
        old: Option<IssueId>,
        new: Option<IssueId>,
    },
    IsPrivate {
        old: bool,
        new: bool,
    },
}

#[derive(Clone)]
pub enum JournalDetail {
    Attr(JournalDetailAttr),
    // FIXME:
    // カスタムフィールド(cf)、添付ファイル(attachment)、リレーション(relation)用のstructの追加
}
