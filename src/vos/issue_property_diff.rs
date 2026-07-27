use chrono::{DateTime, Local};

use crate::vos::{
    ComponentId, IssueId, IssueStatusId, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct IssueSubjectDiff {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueAuthorIdDiff {
    pub before: UserId,
    pub after: UserId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueCreatedOnDiff {
    pub before: DateTime<Local>,
    pub after: DateTime<Local>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueUpdatedOnDiff {
    pub before: DateTime<Local>,
    pub after: DateTime<Local>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueProjectIdDiff {
    pub before: ProjectId,
    pub after: ProjectId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueTrackerIdDiff {
    pub before: TrackerId,
    pub after: TrackerId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueStatusIdDiff {
    pub before: IssueStatusId,
    pub after: IssueStatusId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssuePriorityIdDiff {
    pub before: PriorityId,
    pub after: PriorityId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueAssignedToIdDiff {
    pub before: Option<UserId>,
    pub after: Option<UserId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueFixedVersionDiff {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueTargetVersionIdDiff {
    pub before: Option<TargetVersionId>,
    pub after: Option<TargetVersionId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueStartDateDiff {
    pub before: Option<DateTime<Local>>,
    pub after: Option<DateTime<Local>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueDueDateDiff {
    pub before: Option<DateTime<Local>>,
    pub after: Option<DateTime<Local>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueDoneRatioDiff {
    pub before: u16,
    pub after: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueEstimatedHoursDiff {
    pub before: Option<u16>,
    pub after: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueTotalSpentHoursDiff {
    pub before: Option<f64>,
    pub after: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueResolveWayDiff {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueComponentDiff {
    pub before: Option<ComponentId>,
    pub after: Option<ComponentId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueDescriptionDiff {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueChildIdsDiff {
    pub before: Vec<IssueId>,
    pub after: Vec<IssueId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IssueJournalIdsDiff {
    pub before: Vec<u16>,
    pub after: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IssuePropertyDiff {
    Subject(IssueSubjectDiff),
    AuthorId(IssueAuthorIdDiff),
    CreatedOn(IssueCreatedOnDiff),
    UpdatedOn(IssueUpdatedOnDiff),
    ProjectId(IssueProjectIdDiff),
    TrackerId(IssueTrackerIdDiff),
    StatusId(IssueStatusIdDiff),
    PriorityId(IssuePriorityIdDiff),
    AssignedToId(IssueAssignedToIdDiff),
    TargetVersionId(IssueTargetVersionIdDiff),
    FixedVersion(IssueFixedVersionDiff),
    StartDate(IssueStartDateDiff),
    DueDate(IssueDueDateDiff),
    DoneRatio(IssueDoneRatioDiff),
    EstimatedHours(IssueEstimatedHoursDiff),
    TotalSpentHours(IssueTotalSpentHoursDiff),
    ResolveWay(IssueResolveWayDiff),
    Component(IssueComponentDiff),
    Description(IssueDescriptionDiff),
    ChildIds(IssueChildIdsDiff),
    JournalIds(IssueJournalIdsDiff),
}
