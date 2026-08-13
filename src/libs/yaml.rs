use std::collections::HashMap;
use std::fs;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use yaml_rust::{Yaml, YamlLoader};

use crate::entities::{
    Category, Issue, IssueStatus, Journal, Priority, Project, TargetVersion, TimeEntityActivity,
    Tracker, User,
};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssueStatusId, JournalDetail, JournalDetailAttr,
    PriorityId, ProjectId, TargetVersionId, TimeEntityActivityId, TrackerId, UserId,
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

fn as_string_option(yaml: &Yaml, key: &str) -> Option<String> {
    yaml[key].as_str().map(|s| s.to_string())
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

pub fn parse_journal_yaml(id: u16) -> Journal {
    let path = format!("datas/journals/{}.yml", id);
    let yaml = read_yaml(path.as_str());
    let parsed_id = as_u16(&yaml, "id");
    assert_eq!(
        parsed_id, id,
        "journal id mismatch: {} != {}",
        parsed_id, id
    );
    let user = as_string(&yaml, "user");
    let updated_on = as_local_datetime(&yaml, "updated_on");
    let notes = read_required_string_option(&yaml, "notes").unwrap_or_default();
    let details = yaml["details"]
        .as_vec()
        .expect("no details")
        .iter()
        .map(parse_journal_detail_yaml)
        .collect();

    Journal {
        id: id.into(),
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
            old: as_string(yaml, "old"),
            new: as_string(yaml, "new"),
        },
        "due_date" => JournalDetailAttr::DueDate {
            old: as_local_datetime(yaml, "old"),
            new: as_local_datetime(yaml, "new"),
        },
        "assigned_to" => JournalDetailAttr::AssignedTo {
            old: read_required_string_option(yaml, "old"),
            new: read_required_string_option(yaml, "new"),
        },
        attr_name => panic!("unsupported journal detail attr: {}", attr_name),
    }
}

fn read_required_string_option(yaml: &yaml_rust::Yaml, key: &str) -> Option<String> {
    if yaml[key].is_badvalue() {
        panic!("no {}", key);
    }
    as_string_option(yaml, key)
}

pub fn parse_issue_yaml(id: u16) -> Issue {
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
    let journal_ids = as_u16_array(&yaml, "journal_ids");
    Issue {
        id,
        subject,
        author_id,
        created_on,
        updated_on,
        project_id,
        tracker_id,
        status_id,
        priority_id,
        assigned_to_id,
        target_version_id,
        start_date,
        due_date,
        done_ratio,
        estimated_hours,
        total_spent_hours,
        category_id,
        description,
        child_ids,
        journal_ids,
    }
}

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

pub fn parse_trackers_yaml() -> HashMap<u16, Tracker> {
    let yaml = read_yaml("datas/trackers.yml");
    let entries = yaml["trackers"].as_vec().expect("no trackers");

    entries
        .iter()
        .map(|entry| {
            let tracker = Tracker {
                id: TrackerId::new(as_u16(entry, "id")),
                name: as_string(entry, "name"),
            };
            (tracker.id.get(), tracker)
        })
        .collect()
}

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
            };
            (target_version.id, target_version)
        })
        .collect()
}

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
