//! fixtureのYAMLを読み込み、Redmineのdomain dataへ変換する。

use std::fs;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use yaml_rust::{Yaml, YamlLoader};

use crate::entities::{
    Category, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity, Tracker, User,
};
use crate::entities::{Issue, IssueAggregate, Journal};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId,
    PriorityId, ProjectId, TargetVersionId, TimeEntityActivityId, TrackerId, UserId,
};

fn read_yaml(path: &str) -> Yaml {
    parse_yaml(&read_fixture(path))
}

fn read_fixture(path: &str) -> String {
    fs::read_to_string(path).expect(format!("failed to load {}", path).as_str())
}

fn parse_yaml(yaml: &str) -> Yaml {
    let yaml_all = YamlLoader::load_from_str(yaml).expect("");
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
    let naive_date = NaiveDate::parse_from_str(str, "%Y-%m-%d")
        .expect(format!("failed to parse {} as naive datetime: {}", key, str).as_str());
    let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
    Local.from_local_datetime(&naive_datetime).single().unwrap()
}

fn as_u16_option(yaml: &Yaml, key: &str) -> Option<u16> {
    yaml[key].as_i64().map(|i| i.try_into().unwrap())
}

fn as_f64_option(yaml: &Yaml, key: &str) -> Option<f64> {
    yaml[key]
        .as_f64()
        .or_else(|| yaml[key].as_i64().map(|value| value as f64))
}

fn as_local_datetime_option(yaml: &Yaml, key: &str) -> Option<DateTime<Local>> {
    yaml[key].as_str().map(|s| {
        let naive_date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
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
    let issue_id = IssueId::new(as_u16(&yaml, "issue_id"));
    let user = as_string(&yaml, "user");
    let updated_on = Some(as_local_datetime(&yaml, "updated_on"));
    let notes = yaml["notes"].as_str().unwrap_or_default().to_string();
    let details = yaml["details"]
        .as_vec()
        .expect("no details")
        .iter()
        .map(parse_journal_detail_yaml)
        .collect();

    Journal {
        id,
        issue_id,
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
            old: as_f64_option(yaml, "old"),
            new: as_f64_option(yaml, "new"),
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
    let estimated_hours = as_f64_option(&yaml, "estimated_hours");
    let total_spent_hours = as_f64_option(&yaml, "total_spent_hours");
    let category_id = as_u16_option(&yaml, "category_id").map(CategoryId::new);
    let description = as_string(&yaml, "description");
    let child_ids = as_u16_array(&yaml, "child_ids")
        .into_iter()
        .map(IssueId::new)
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
    }
}

fn parse_master_entries<T>(yaml: &Yaml, key: &str, parse: impl Fn(&Yaml) -> T) -> Vec<T> {
    yaml[key]
        .as_vec()
        .expect(format!("no {}", key).as_str())
        .iter()
        .map(parse)
        .collect()
}

pub fn parse_users(yaml: &str) -> Vec<User> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "users", |entry| User {
        id: UserId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
    })
}

pub fn parse_issue_statuses(yaml: &str) -> Vec<IssueStatus> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "issue_statuses", |entry| IssueStatus {
        id: IssueStatusId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
        is_closed: as_bool(entry, "is_closed"),
    })
}

pub fn parse_priorities(yaml: &str) -> Vec<Priority> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "priorities", |entry| Priority {
        id: PriorityId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
    })
}

pub fn parse_projects(yaml: &str) -> Vec<Project> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "projects", |entry| Project {
        id: ProjectId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
    })
}

pub fn parse_trackers(yaml: &str) -> Vec<Tracker> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "trackers", |entry| Tracker {
        id: TrackerId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
    })
}

pub fn parse_target_versions(yaml: &str) -> Vec<TargetVersion> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "target_versions", |entry| TargetVersion {
        id: TargetVersionId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
        project_id: ProjectId::new(as_u16(entry, "project_id")),
    })
}

pub fn parse_categories(yaml: &str) -> Vec<Category> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "categories", |entry| Category {
        id: CategoryId::new(as_u16(entry, "id")),
        name: as_string(entry, "name"),
        project_id: ProjectId::new(as_u16(entry, "project_id")),
    })
}

pub fn parse_time_entity_activities(yaml: &str) -> Vec<TimeEntityActivity> {
    let yaml = parse_yaml(yaml);
    parse_master_entries(&yaml, "time_entity_activities", |entry| {
        TimeEntityActivity {
            id: TimeEntityActivityId::new(as_u16(entry, "id")),
            name: as_string(entry, "name"),
            is_default: as_bool(entry, "is_default"),
        }
    })
}

#[cfg(test)]
pub fn parse_users_yaml() -> Vec<User> {
    parse_users(&read_fixture("datas/users.yml"))
}

#[cfg(test)]
pub fn parse_issue_statuses_yaml() -> Vec<IssueStatus> {
    parse_issue_statuses(&read_fixture("datas/issue_statuses.yml"))
}

#[cfg(test)]
pub fn parse_priorities_yaml() -> Vec<Priority> {
    parse_priorities(&read_fixture("datas/priorities.yml"))
}

#[cfg(test)]
pub fn parse_projects_yaml() -> Vec<Project> {
    parse_projects(&read_fixture("datas/projects.yml"))
}

#[cfg(test)]
pub fn parse_trackers_yaml() -> Vec<Tracker> {
    parse_trackers(&read_fixture("datas/trackers.yml"))
}

#[cfg(test)]
pub fn parse_target_versions_yaml() -> Vec<TargetVersion> {
    parse_target_versions(&read_fixture("datas/target_versions.yml"))
}

#[cfg(test)]
pub fn parse_categories_yaml() -> Vec<Category> {
    parse_categories(&read_fixture("datas/categories.yml"))
}

#[cfg(test)]
pub fn parse_time_entity_activities_yaml() -> Vec<TimeEntityActivity> {
    parse_time_entity_activities(&read_fixture("datas/time_entity_activities.yml"))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_categories, parse_issue_statuses, parse_journal_yaml, parse_priorities,
        parse_projects, parse_target_versions, parse_time_entity_activities, parse_trackers,
        parse_users,
    };
    use crate::vos::{IssueId, IssueStatusId, JournalDetail, JournalDetailAttr, JournalId};

    #[test]
    fn parse_master_fixtures_preserves_order_and_values() {
        let users = parse_users(include_str!("../../datas/users.yml"));
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "user1");

        let statuses = parse_issue_statuses(include_str!("../../datas/issue_statuses.yml"));
        assert_eq!(statuses.len(), 6);
        assert_eq!(statuses[4].name, "完了(closed)");
        assert!(statuses[4].is_closed);

        let priorities = parse_priorities(include_str!("../../datas/priorities.yml"));
        assert_eq!(priorities.len(), 4);
        assert_eq!(priorities[0].name, "major");

        let projects = parse_projects(include_str!("../../datas/projects.yml"));
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "Sample Project");

        let trackers = parse_trackers(include_str!("../../datas/trackers.yml"));
        assert_eq!(trackers.len(), 3);
        assert_eq!(trackers[1].name, "Feature");

        let versions = parse_target_versions(include_str!("../../datas/target_versions.yml"));
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].name, "v1.2.3");

        let categories = parse_categories(include_str!("../../datas/categories.yml"));
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].name, "category1");

        let activities =
            parse_time_entity_activities(include_str!("../../datas/time_entity_activities.yml"));
        assert_eq!(activities.len(), 3);
        assert_eq!(activities[0].name, "設計");
        assert!(activities[0].is_default);
    }

    #[test]
    fn parse_journal_yaml_reads_all_fields_from_fixture() {
        assert_journal_1();
        assert_journal_2();
        assert_journal_3();
    }

    fn assert_journal_1() {
        let journal = parse_journal_yaml(JournalId::new(1));
        assert_eq!(journal.id, JournalId::new(1));
        assert_eq!(journal.issue_id, IssueId::new(3));
        assert_eq!(journal.user, "user1");
        assert_eq!(
            journal
                .updated_on
                .expect("updated_on should be set")
                .format("%Y-%m-%d")
                .to_string(),
            "2026-02-10"
        );
        assert_eq!(journal.notes, "");
        assert_eq!(journal.details.len(), 1);
        match &journal.details[0] {
            JournalDetail::Attr(JournalDetailAttr::StatusId { old, new }) => {
                assert_eq!(*old, IssueStatusId::new(1));
                assert_eq!(*new, IssueStatusId::new(2));
            }
            _ => panic!("journal 1 should have a status_id detail"),
        }
    }

    fn assert_journal_2() {
        let journal = parse_journal_yaml(JournalId::new(2));
        assert_eq!(journal.id, JournalId::new(2));
        assert_eq!(journal.issue_id, IssueId::new(3));
        assert_eq!(journal.user, "user1");
        assert_eq!(
            journal
                .updated_on
                .expect("updated_on should be set")
                .format("%Y-%m-%d")
                .to_string(),
            "2026-02-16"
        );
        assert_eq!(journal.notes, "");
        assert_eq!(journal.details.len(), 1);
        match &journal.details[0] {
            JournalDetail::Attr(JournalDetailAttr::DueDate { old, new }) => {
                let old = old.as_ref().expect("due_date.old should be set");
                let new = new.as_ref().expect("due_date.new should be set");
                assert_eq!(old.format("%Y-%m-%d").to_string(), "2026-02-16");
                assert_eq!(new.format("%Y-%m-%d").to_string(), "2026-02-17");
            }
            _ => panic!("journal 2 should have a due_date detail"),
        }
    }

    fn assert_journal_3() {
        let journal = parse_journal_yaml(JournalId::new(3));
        assert_eq!(journal.id, JournalId::new(3));
        assert_eq!(journal.issue_id, IssueId::new(3));
        assert_eq!(journal.user, "user1");
        assert_eq!(
            journal
                .updated_on
                .expect("updated_on should be set")
                .format("%Y-%m-%d")
                .to_string(),
            "2026-02-16"
        );
        assert!(!journal.notes.is_empty());
        assert!(journal.notes.starts_with("### h3"));
        assert!(journal.notes.ends_with("> citation"));
        assert!(journal.notes.contains("*italic text*"));
        assert!(journal.notes.contains("**bold text**"));
        assert!(journal.notes.contains("1. numbered list 1"));
        assert!(journal.notes.contains("inner numbered list 1"));
        assert!(journal.notes.contains("- itemized list 1"));
        assert!(journal.notes.contains("~~canceled text~~"));
        assert!(journal.notes.contains("`code`"));
        assert!(journal.notes.contains("code block"));
        assert!(journal.details.is_empty());
    }
}
