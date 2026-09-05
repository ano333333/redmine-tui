use serde::Deserialize;

use super::journal_detail_value_conversion::{
    parse_bool, parse_optional_calendar_date, parse_optional_u16, parse_optional_whole_hours,
    parse_required_u16,
};
use crate::clients::redmine::RedmineClientError;
use crate::vos::{
    CategoryId, IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, PriorityId, ProjectId,
    TargetVersionId, TrackerId, UserId,
};

#[derive(Deserialize)]
pub(super) struct RedmineJournalDetail {
    pub(super) property: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) old_value: Option<String>,
    #[serde(default)]
    pub(super) new_value: Option<String>,
}

impl RedmineJournalDetail {
    pub(super) fn try_into_domain(self) -> Result<Option<JournalDetail>, RedmineClientError> {
        if self.property != "attr" {
            match self.property.as_str() {
                "cf" | "attachment" | "relation" => {
                    // FIXME: Preserve these details after adding their corresponding VO/variants.
                }
                // Redmine may add detail properties independently of this client.
                _ => {}
            }
            return Ok(None);
        }

        let old = self.old_value;
        let new = self.new_value;
        let required = |value: &Option<String>| parse_required_u16(value.as_deref());
        let optional = |value: &Option<String>| parse_optional_u16(value.as_deref());
        let attr = match self.name.as_str() {
            "status_id" => JournalDetailAttr::StatusId {
                old: IssueStatusId::new(required(&old)?),
                new: IssueStatusId::new(required(&new)?),
            },
            "tracker_id" => JournalDetailAttr::TrackerId {
                old: TrackerId::new(required(&old)?),
                new: TrackerId::new(required(&new)?),
            },
            "project_id" => JournalDetailAttr::ProjectId {
                old: ProjectId::new(required(&old)?),
                new: ProjectId::new(required(&new)?),
            },
            "subject" => JournalDetailAttr::Subject {
                old: old.unwrap_or_default(),
                new: new.unwrap_or_default(),
            },
            "description" => JournalDetailAttr::Description {
                old: old.unwrap_or_default(),
                new: new.unwrap_or_default(),
            },
            "category_id" => JournalDetailAttr::CategoryId {
                old: optional(&old)?.map(CategoryId::new),
                new: optional(&new)?.map(CategoryId::new),
            },
            "assigned_to_id" => JournalDetailAttr::AssignedToId {
                old: optional(&old)?.map(UserId::new),
                new: optional(&new)?.map(UserId::new),
            },
            "priority_id" => JournalDetailAttr::PriorityId {
                old: PriorityId::new(required(&old)?),
                new: PriorityId::new(required(&new)?),
            },
            "fixed_version_id" => JournalDetailAttr::FixedVersionId {
                old: optional(&old)?.map(TargetVersionId::new),
                new: optional(&new)?.map(TargetVersionId::new),
            },
            "author_id" => JournalDetailAttr::AuthorId {
                old: UserId::new(required(&old)?),
                new: UserId::new(required(&new)?),
            },
            "start_date" => JournalDetailAttr::StartDate {
                old: parse_optional_calendar_date(old.as_deref())?,
                new: parse_optional_calendar_date(new.as_deref())?,
            },
            "due_date" => JournalDetailAttr::DueDate {
                old: parse_optional_calendar_date(old.as_deref())?,
                new: parse_optional_calendar_date(new.as_deref())?,
            },
            "done_ratio" => JournalDetailAttr::DoneRatio {
                old: required(&old)?,
                new: required(&new)?,
            },
            "estimated_hours" => JournalDetailAttr::EstimatedHours {
                old: parse_optional_whole_hours(old.as_deref())?,
                new: parse_optional_whole_hours(new.as_deref())?,
            },
            "parent_id" => JournalDetailAttr::ParentId {
                old: optional(&old)?.map(IssueId::new),
                new: optional(&new)?.map(IssueId::new),
            },
            "is_private" => JournalDetailAttr::IsPrivate {
                old: parse_bool(old.as_deref())?,
                new: parse_bool(new.as_deref())?,
            },
            name => {
                return Err(RedmineClientError::Client {
                    reason: format!("unsupported journal detail attr '{name}'"),
                });
            }
        };
        Ok(Some(JournalDetail::Attr(attr)))
    }
}
