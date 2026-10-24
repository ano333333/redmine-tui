use super::{block_on, mount_get};
use crate::clients::redmine::{DefaultRedmineClient, RedmineClient};
use crate::vos::{IssueId, JournalDetail, JournalDetailAttr, JournalId};

#[test]
fn get_issue_skips_unsupported_journal_detail_properties() {
    let mock_server = block_on(wiremock::MockServer::start());
    mount_get(
        &mock_server,
        "/issues/42.json",
        200,
        r#"{
            "issue": {
                "id": 42,
                "subject": "Fix login",
                "author": {"id": 1000},
                "created_on": "2026-08-18T12:00:00Z",
                "updated_on": "2026-08-18T13:00:00Z",
                "project": {"id": 10},
                "tracker": {"id": 2},
                "status": {"id": 1},
                "priority": {"id": 5},
                "done_ratio": 30,
                "journals": [
                    {
                        "id": 500,
                        "user": {"id": 1000, "name": "Taro Yamada"},
                        "updated_on": "2026-08-18T14:00:00Z",
                        "notes": "first journal",
                        "details": [
                            {"property": "attr", "name": "status_id", "old_value": "1", "new_value": "2"},
                            {"property": "cf_1", "name": "custom1", "old_value": null, "new_value": "custom value"},
                            {"property": "relation", "name": "43", "relation_type": "child"}
                        ]
                    }
                ]
            }
        }"#,
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let fetched = block_on(client.get_issue(IssueId::new(42))).unwrap();

    assert_eq!(fetched.aggregate.journals.len(), 1);
    assert_eq!(fetched.aggregate.journals[0].id, JournalId::new(500));
    assert_eq!(fetched.aggregate.journals[0].issue_id, IssueId::new(42));
    assert_eq!(fetched.aggregate.journals[0].details.len(), 1);
    match &fetched.aggregate.journals[0].details[0] {
        JournalDetail::Attr(JournalDetailAttr::StatusId { old, new }) => {
            assert_eq!(*old, crate::vos::IssueStatusId::new(1));
            assert_eq!(*new, crate::vos::IssueStatusId::new(2));
        }
        _ => panic!("unsupported details were not skipped"),
    }
}

#[test]
fn get_issue_requests_and_converts_attachments() {
    let mock_server = block_on(wiremock::MockServer::start());
    block_on(
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/issues/42.json"))
            .and(wiremock::matchers::query_param(
                "include",
                "children,journals,attachments",
            ))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{
                    "issue": {
                        "id": 42,
                        "subject": "Fix login",
                        "author": {"id": 1000},
                        "created_on": "2026-08-18T12:00:00Z",
                        "updated_on": "2026-08-18T13:00:00Z",
                        "project": {"id": 10},
                        "tracker": {"id": 2},
                        "status": {"id": 1},
                        "priority": {"id": 5},
                        "done_ratio": 30,
                        "attachments": [
                            {
                                "id": 7,
                                "filename": "first.png",
                                "filesize": 100,
                                "content_type": "image/png",
                                "description": "",
                                "content_url": "http://redmine.test/attachments/download/7/first.png",
                                "author": {"id": 1000, "name": "Taro Yamada"},
                                "created_on": "2026-08-18T12:30:00Z"
                            },
                            {
                                "id": 8,
                                "filename": "second.txt",
                                "filesize": 200,
                                "content_type": "text/plain",
                                "description": "log",
                                "content_url": "http://redmine.test/attachments/download/8/second.txt",
                                "author": {"id": 1000, "name": "Taro Yamada"},
                                "created_on": "2026-08-18T12:40:00Z"
                            }
                        ]
                    }
                }"#,
            ))
            .expect(1)
            .mount(&mock_server),
    );
    let client = DefaultRedmineClient::new(mock_server.uri(), "secret-token");

    let fetched = block_on(client.get_issue(IssueId::new(42))).unwrap();

    let filenames: Vec<_> = fetched
        .aggregate
        .attachments
        .iter()
        .map(|attachment| attachment.filename.as_str())
        .collect();
    assert_eq!(filenames, vec!["first.png", "second.txt"]);
}
