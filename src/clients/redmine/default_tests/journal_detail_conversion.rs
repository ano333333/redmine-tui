use crate::clients::redmine::RedmineClientError;
use crate::clients::redmine::default::journal_detail_conversion::RedmineJournalDetail;
use crate::vos::{JournalDetail, JournalDetailAttr};

fn attr(name: &str, old: Option<&str>, new: Option<&str>) -> JournalDetailAttr {
    let detail = RedmineJournalDetail {
        property: "attr".into(),
        name: name.into(),
        old_value: old.map(str::to_string),
        new_value: new.map(str::to_string),
    };
    let Some(JournalDetail::Attr(attr)) = detail.try_into_domain().unwrap() else {
        panic!("attr detail must be retained");
    };
    attr
}

#[test]
fn maps_every_attr_variant_with_old_and_new_values() {
    macro_rules! id_pair {
        ($name:literal, $variant:path, $id:path) => {
            let $variant { old, new } = attr($name, Some("1"), Some("2")) else {
                panic!()
            };
            assert_eq!((old, new), ($id(1), $id(2)));
        };
    }
    id_pair!(
        "status_id",
        JournalDetailAttr::StatusId,
        crate::vos::IssueStatusId::new
    );
    id_pair!(
        "tracker_id",
        JournalDetailAttr::TrackerId,
        crate::vos::TrackerId::new
    );
    id_pair!(
        "project_id",
        JournalDetailAttr::ProjectId,
        crate::vos::ProjectId::new
    );
    id_pair!(
        "priority_id",
        JournalDetailAttr::PriorityId,
        crate::vos::PriorityId::new
    );
    id_pair!(
        "author_id",
        JournalDetailAttr::AuthorId,
        crate::vos::UserId::new
    );

    let JournalDetailAttr::Subject { old, new } = attr("subject", None, Some("new")) else {
        panic!()
    };
    assert_eq!((old.as_str(), new.as_str()), ("", "new"));
    let JournalDetailAttr::Description { old, new } = attr("description", Some("old"), None) else {
        panic!()
    };
    assert_eq!((old.as_str(), new.as_str()), ("old", ""));

    let JournalDetailAttr::CategoryId { old, new } = attr("category_id", None, Some("2")) else {
        panic!()
    };
    assert_eq!((old, new), (None, Some(crate::vos::CategoryId::new(2))));
    let JournalDetailAttr::AssignedToId { old, new } = attr("assigned_to_id", Some("1"), Some(""))
    else {
        panic!()
    };
    assert_eq!((old, new), (Some(crate::vos::UserId::new(1)), None));
    let JournalDetailAttr::FixedVersionId { old, new } =
        attr("fixed_version_id", Some("1"), Some("2"))
    else {
        panic!()
    };
    assert_eq!(
        (old, new),
        (
            Some(crate::vos::TargetVersionId::new(1)),
            Some(crate::vos::TargetVersionId::new(2))
        )
    );
    let JournalDetailAttr::ParentId { old, new } = attr("parent_id", Some(""), Some("2")) else {
        panic!()
    };
    assert_eq!((old, new), (None, Some(crate::vos::IssueId::new(2))));

    let JournalDetailAttr::StartDate { old, new } = attr("start_date", None, Some("2026-09-05"))
    else {
        panic!()
    };
    assert!(old.is_none());
    assert_eq!(new.unwrap().format("%Y-%m-%d").to_string(), "2026-09-05");
    let JournalDetailAttr::DueDate { old, new } =
        attr("due_date", Some("2026-09-04"), Some("2026-09-05"))
    else {
        panic!()
    };
    assert_eq!(old.unwrap().format("%Y-%m-%d").to_string(), "2026-09-04");
    assert_eq!(new.unwrap().format("%Y-%m-%d").to_string(), "2026-09-05");
    let JournalDetailAttr::DoneRatio { old, new } = attr("done_ratio", Some("10"), Some("20"))
    else {
        panic!()
    };
    assert_eq!((old, new), (10, 20));
    let JournalDetailAttr::EstimatedHours { old, new } = attr("estimated_hours", Some("1.0"), None)
    else {
        panic!()
    };
    assert_eq!((old, new), (Some(1), None));
    let JournalDetailAttr::IsPrivate { old, new } = attr("is_private", Some("0"), Some("1")) else {
        panic!()
    };
    assert_eq!((old, new), (false, true));
}

#[test]
fn skips_non_attr_properties_including_known_unsupported_kinds() {
    for property in ["cf", "attachment", "relation", "future_property"] {
        let detail = RedmineJournalDetail {
            property: property.into(),
            name: "ignored".into(),
            old_value: None,
            new_value: None,
        };
        assert!(detail.try_into_domain().unwrap().is_none());
    }
}

#[test]
fn rejects_unknown_attr_invalid_values_and_missing_required_values() {
    for (name, old, new) in [
        ("unknown", Some("1"), Some("2")),
        ("status_id", Some("invalid"), Some("2")),
        ("status_id", None, Some("2")),
        ("due_date", Some("2026-9-5"), Some("2026-09-05")),
    ] {
        let detail = RedmineJournalDetail {
            property: "attr".into(),
            name: name.into(),
            old_value: old.map(str::to_string),
            new_value: new.map(str::to_string),
        };
        assert!(matches!(
            detail.try_into_domain(),
            Err(RedmineClientError::Client { .. })
        ));
    }
}
