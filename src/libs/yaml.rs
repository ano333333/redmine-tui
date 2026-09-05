use std::fs;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use yaml_rust::{Yaml, YamlLoader};

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
use crate::entities::{
    Category, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity, Tracker, User,
};
use crate::entities::{Issue, IssueAggregate, Journal};
#[cfg(test)]
use crate::vos::TimeEntityActivityId;
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId,
    JournalKey, PriorityId, ProjectId, TargetVersionId, TrackerId, UserId,
};

fn read_yaml(path: &str) -> Yaml {
    let yaml_all = fs::read_to_string(path).expect(format!("failed to load {}", path).as_str());
    let yaml_all = YamlLoader::load_from_str(yaml_all.as_str()).expect("");
    yaml_all.iter().next().unwrap().clone()
}

fn as_u16(yaml: &Yaml, key: &str) -> u16 {
    yaml[key]
        .as_i64()
        .expect(format!("no {}", key).as_str())
        .try_into()
        .unwrap()
}

fn as_string(yaml: &Yaml, key: &str) -> String {
    yaml[key]
        .as_str()
        .expect(format!("no {}", key).as_str())
        .to_string()
}

fn as_bool(yaml: &Yaml, key: &str) -> bool {
    yaml[key].as_bool().expect(format!("no {}", key).as_str())
}

fn as_local_datetime(yaml: &Yaml, key: &str) -> DateTime<Local> {
    let str = yaml[key].as_str().expect(format!("no {}", key).as_str());
    let naive_date = NaiveDate::parse_from_str(str, "%Y/%m/%d")
        .expect(format!("failed to parse {} as naive datetime: {}", key, str).as_str());
    let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
    Local.from_local_datetime(&naive_datetime).single().unwrap()
}

fn as_u16_option(yaml: &Yaml, key: &str) -> Option<u16> {
    yaml[key].as_i64().map(|i| i.try_into().unwrap())
}

fn as_f64_option(yaml: &Yaml, key: &str) -> Option<f64> {
    yaml[key].as_f64()
}

fn as_local_datetime_option(yaml: &Yaml, key: &str) -> Option<DateTime<Local>> {
    yaml[key].as_str().map(|s| {
        let naive_date = NaiveDate::parse_from_str(s, "%Y/%m/%d")
            .expect(format!("failed to parse {} as naive datetime: {}", key, s).as_str());
        let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        Local.from_local_datetime(&naive_datetime).single().unwrap()
    })
}

fn as_u16_array(yaml: &Yaml, key: &str) -> Vec<u16> {
    let mut res = Vec::<u16>::new();
    if let Some(v) = yaml[key].as_vec() {
        for s in v {
            res.push(s.as_i64().unwrap().try_into().unwrap());
        }
    }
    res
}

pub fn parse_journal_yaml(id: JournalId) -> Journal {
    let path = format!("datas/journals/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let parsed_id = as_u16(&yaml, "id");
    assert_eq!(
        parsed_id,
        id.get(),
        "journal id mismatch: {} != {}",
        parsed_id,
        id
    );
    let user = as_string(&yaml, "user");
    let updated_on = as_local_datetime(&yaml, "updated_on");
    let notes = yaml["notes"].as_str().unwrap_or_default().to_string();
    let details = yaml["details"]
        .as_vec()
        .expect("no details")
        .iter()
        .map(parse_journal_detail_yaml)
        .collect();

    Journal {
        id,
        user,
        updated_on,
        details,
        notes,
    }
}

pub fn parse_journal_detail_yaml(yaml: &yaml_rust::Yaml) -> JournalDetail {
    match as_string(yaml, "type").as_str() {
        "attr" => JournalDetail::Attr(parse_journal_detail_attr_yaml(yaml)),
        detail_type => panic!("unsupported journal detail type: {}", detail_type),
    }
}

pub fn parse_journal_detail_attr_yaml(yaml: &yaml_rust::Yaml) -> JournalDetailAttr {
    match as_string(yaml, "name").as_str() {
        "status_id" => JournalDetailAttr::StatusId {
            old: IssueStatusId::new(as_u16(yaml, "old")),
            new: IssueStatusId::new(as_u16(yaml, "new")),
        },
        "tracker_id" => JournalDetailAttr::TrackerId {
            old: TrackerId::new(as_u16(yaml, "old")),
            new: TrackerId::new(as_u16(yaml, "new")),
        },
        "project_id" => JournalDetailAttr::ProjectId {
            old: ProjectId::new(as_u16(yaml, "old")),
            new: ProjectId::new(as_u16(yaml, "new")),
        },
        "subject" => JournalDetailAttr::Subject {
            old: as_string(yaml, "old"),
            new: as_string(yaml, "new"),
        },
        "description" => JournalDetailAttr::Description {
            old: as_string(yaml, "old"),
            new: as_string(yaml, "new"),
        },
        "category_id" => JournalDetailAttr::CategoryId {
            old: as_u16_option(yaml, "old").map(CategoryId::new),
            new: as_u16_option(yaml, "new").map(CategoryId::new),
        },
        "assigned_to_id" => JournalDetailAttr::AssignedToId {
            old: as_u16_option(yaml, "old").map(UserId::new),
            new: as_u16_option(yaml, "new").map(UserId::new),
        },
        "priority_id" => JournalDetailAttr::PriorityId {
            old: PriorityId::new(as_u16(yaml, "old")),
            new: PriorityId::new(as_u16(yaml, "new")),
        },
        "fixed_version_id" => JournalDetailAttr::FixedVersionId {
            old: as_u16_option(yaml, "old").map(TargetVersionId::new),
            new: as_u16_option(yaml, "new").map(TargetVersionId::new),
        },
        "author_id" => JournalDetailAttr::AuthorId {
            old: UserId::new(as_u16(yaml, "old")),
            new: UserId::new(as_u16(yaml, "new")),
        },
        "start_date" => JournalDetailAttr::StartDate {
            old: as_local_datetime_option(yaml, "old"),
            new: as_local_datetime_option(yaml, "new"),
        },
        "due_date" => JournalDetailAttr::DueDate {
            old: as_local_datetime_option(yaml, "old"),
            new: as_local_datetime_option(yaml, "new"),
        },
        "done_ratio" => JournalDetailAttr::DoneRatio {
            old: as_u16(yaml, "old"),
            new: as_u16(yaml, "new"),
        },
        "estimated_hours" => JournalDetailAttr::EstimatedHours {
            old: as_u16_option(yaml, "old"),
            new: as_u16_option(yaml, "new"),
        },
        "parent_id" => JournalDetailAttr::ParentId {
            old: as_u16_option(yaml, "old").map(IssueId::new),
            new: as_u16_option(yaml, "new").map(IssueId::new),
        },
        "is_private" => JournalDetailAttr::IsPrivate {
            old: as_bool(yaml, "old"),
            new: as_bool(yaml, "new"),
        },
        attr_name => panic!("unsupported journal detail attr: {}", attr_name),
    }
}

pub fn parse_issue_yaml(id: u16) -> IssueAggregate {
    let path = format!("datas/issues/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let id = IssueId::new(as_u16(&yaml, "id"));
    let subject = as_string(&yaml, "subject");
    let author_id = UserId::new(as_u16(&yaml, "author_id"));
    let created_on = as_local_datetime(&yaml, "created_on");
    let updated_on = as_local_datetime(&yaml, "updated_on");
    let project_id = ProjectId::new(as_u16(&yaml, "project_id"));
    let tracker_id = TrackerId::new(as_u16(&yaml, "tracker_id"));
    let status_id = IssueStatusId::new(as_u16(&yaml, "status_id"));
    let priority_id = PriorityId::new(as_u16(&yaml, "priority_id"));
    let assigned_to_id = as_u16_option(&yaml, "assigned_to_id").map(UserId::new);
    let target_version_id = as_u16_option(&yaml, "target_version_id").map(TargetVersionId::new);
    let start_date = as_local_datetime_option(&yaml, "start_date");
    let due_date = as_local_datetime_option(&yaml, "due_date");
    let done_ratio = as_u16(&yaml, "done_ratio");
    let estimated_hours = as_u16_option(&yaml, "estimated_hours");
    let total_spent_hours = as_f64_option(&yaml, "total_spent_hours");
    let category_id = as_u16_option(&yaml, "category_id").map(CategoryId::new);
    let description = as_string(&yaml, "description");
    let child_ids = as_u16_array(&yaml, "child_ids")
        .into_iter()
        .map(IssueId::new)
        .collect();
    let journal_keys = as_u16_array(&yaml, "journal_ids")
        .iter()
        .map(|id| JournalKey::Remote(JournalId::new(*id)))
        .collect();
    IssueAggregate {
        issue: Issue {
            id,
            project_id,
            subject: subject.clone(),
            description: description.clone(),
            status_id,
        },
        author_id,
        created_on,
        updated_on,
        tracker_id,
        priority_id,
        assigned_to_id,
        target_version_id,
        start_date,
        due_date,
        done_ratio,
        estimated_hours,
        total_spent_hours,
        category_id,
        child_ids,
        journal_keys,
    }
}

#[cfg(test)]
pub fn parse_users_yaml() -> HashMap<UserId, User> {
    let yaml = read_yaml("datas/users.yml");
    let entries = yaml["users"].as_vec().expect("no users");

    entries
        .iter()
        .map(|entry| {
            let user = User {
                id: UserId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (user.id, user)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_issue_statuses_yaml() -> HashMap<IssueStatusId, IssueStatus> {
    let yaml = read_yaml("datas/issue_statuses.yml");
    let entries = yaml["issue_statuses"].as_vec().expect("no issue_statuses");

    entries
        .iter()
        .map(|entry| {
            let status = IssueStatus {
                id: IssueStatusId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
                is_closed: as_bool(entry, "is_closed"),
            };
            (status.id, status)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_priorities_yaml() -> HashMap<PriorityId, Priority> {
    let yaml = read_yaml("datas/priorities.yml");
    let entries = yaml["priorities"].as_vec().expect("no priorities");

    entries
        .iter()
        .map(|entry| {
            let priority = Priority {
                id: PriorityId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (priority.id, priority)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_projects_yaml() -> HashMap<ProjectId, Project> {
    let yaml = read_yaml("datas/projects.yml");
    let entries = yaml["projects"].as_vec().expect("no projects");

    entries
        .iter()
        .map(|entry| {
            let project = Project {
                id: ProjectId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (project.id, project)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_trackers_yaml() -> HashMap<TrackerId, Tracker> {
    let yaml = read_yaml("datas/trackers.yml");
    let entries = yaml["trackers"].as_vec().expect("no trackers");

    entries
        .iter()
        .map(|entry| {
            let tracker = Tracker {
                id: TrackerId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (tracker.id, tracker)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_target_versions_yaml() -> HashMap<TargetVersionId, TargetVersion> {
    let yaml = read_yaml("datas/target_versions.yml");
    let entries = yaml["target_versions"]
        .as_vec()
        .expect("no target_versions");

    entries
        .iter()
        .map(|entry| {
            let target_version = TargetVersion {
                id: TargetVersionId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
                project_id: ProjectId::new(as_u16(entry, "project_id")),
            };
            (target_version.id, target_version)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_categories_yaml() -> HashMap<CategoryId, Category> {
    let yaml = read_yaml("datas/categories.yml");
    let entries = yaml["categories"].as_vec().expect("no categories");

    entries
        .iter()
        .map(|entry| {
            let category = Category {
                id: CategoryId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (category.id, category)
        })
        .collect()
}

#[cfg(test)]
pub fn parse_time_entity_activities_yaml() -> HashMap<TimeEntityActivityId, TimeEntityActivity> {
    let yaml = read_yaml("datas/time_entity_activities.yml");
    let entries = yaml["time_entity_activities"]
        .as_vec()
        .expect("no time_entity_activities");

    entries
        .iter()
        .map(|entry| {
            let act = TimeEntityActivity {
                id: TimeEntityActivityId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
                is_default: as_bool(entry, "is_default"),
            };
            (act.id, act)
        })
        .collect()
}
