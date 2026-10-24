//! Webデモの埋め込みfixtureから、memory serverの初期状態を構築する。

use std::collections::BTreeMap;

use crate::entities::{
    Attachment, Category, IssueAggregate, IssueStatus, Journal, Priority, Project, TargetVersion,
    TimeEntityActivity, Tracker, User,
};
use crate::libs::yaml;
use crate::vos::{AttachmentId, IssueId, JournalId};

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/datas/", $path))
    };
}

const USERS: &str = fixture!("users.yml");
const ISSUE_STATUSES: &str = fixture!("issue_statuses.yml");
const PRIORITIES: &str = fixture!("priorities.yml");
const PROJECTS: &str = fixture!("projects.yml");
const TRACKERS: &str = fixture!("trackers.yml");
const TARGET_VERSIONS: &str = fixture!("target_versions.yml");
const CATEGORIES: &str = fixture!("categories.yml");
const TIME_ENTITY_ACTIVITIES: &str = fixture!("time_entity_activities.yml");
// FIXME: Issue/Journal/添付ファイルのfixtureファイルを手書きで列挙しているため、追加・削除時にcatalogの更新が必要。build時の列挙などで自動化する。
const ISSUES: &[(IssueId, &str, &[(JournalId, &str)])] = &[
    (IssueId::new(1), fixture!("issues/1.yml"), &[]),
    (IssueId::new(2), fixture!("issues/2.yml"), &[]),
    (
        IssueId::new(3),
        fixture!("issues/3.yml"),
        &[
            (JournalId::new(1), fixture!("journals/1.yml")),
            (JournalId::new(2), fixture!("journals/2.yml")),
            (JournalId::new(3), fixture!("journals/3.yml")),
        ],
    ),
    (IssueId::new(4), fixture!("issues/4.yml"), &[]),
];
// Webデモのwasmを小さくするため、seedの`sample.png`ではなく縮小版の大きさを使う。
const ATTACHMENTS: &[(AttachmentId, IssueId, &str, &[u8])] = &[(
    AttachmentId::new(1),
    IssueId::new(4),
    fixture!("attachments/1.yml"),
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/datas/attachment_files/sample_web.png"
    )),
)];

pub struct DemoFixtureState {
    pub users: Vec<User>,
    pub issue_statuses: Vec<IssueStatus>,
    pub priorities: Vec<Priority>,
    pub projects: Vec<Project>,
    pub trackers: Vec<Tracker>,
    pub target_versions: Vec<TargetVersion>,
    pub categories: Vec<Category>,
    pub time_entity_activities: Vec<TimeEntityActivity>,
    pub issues: BTreeMap<IssueId, IssueAggregate>,
    pub journals: BTreeMap<IssueId, Vec<Journal>>,
    pub attachments: BTreeMap<IssueId, Vec<Attachment>>,
    /// uploadしたがIssueへ紐付けていないファイル。キーはtoken。
    pub pending_uploads: BTreeMap<String, PendingUpload>,
}

pub struct PendingUpload {
    /// Redmineと同じく、upload時点で添付ファイルIDを確保する。
    pub id: AttachmentId,
    pub filesize: u64,
}

impl DemoFixtureState {
    /// 呼び出すたびにfixtureを再解析し、デモの変更を引き継がない初期状態を返す。
    pub fn initial() -> Self {
        let (issues, journals) = ISSUES.iter().fold(
            (BTreeMap::new(), BTreeMap::new()),
            |(mut issues, mut journals), (id, fixture, journal_fixtures)| {
                issues.insert(*id, yaml::parse_issue(*id, fixture));
                journals.insert(
                    *id,
                    journal_fixtures
                        .iter()
                        .map(|(journal_id, journal_fixture)| {
                            yaml::parse_journal(*journal_id, *id, journal_fixture)
                        })
                        .collect(),
                );
                (issues, journals)
            },
        );
        let attachments = ATTACHMENTS.iter().fold(
            BTreeMap::<IssueId, Vec<Attachment>>::new(),
            |mut attachments, (id, issue_id, fixture, file)| {
                attachments
                    .entry(*issue_id)
                    .or_default()
                    .push(yaml::parse_attachment(
                        *id,
                        *issue_id,
                        fixture,
                        file.len() as u64,
                    ));
                attachments
            },
        );
        Self {
            users: yaml::parse_users(USERS),
            issue_statuses: yaml::parse_issue_statuses(ISSUE_STATUSES),
            priorities: yaml::parse_priorities(PRIORITIES),
            projects: yaml::parse_projects(PROJECTS),
            trackers: yaml::parse_trackers(TRACKERS),
            target_versions: yaml::parse_target_versions(TARGET_VERSIONS),
            categories: yaml::parse_categories(CATEGORIES),
            time_entity_activities: yaml::parse_time_entity_activities(TIME_ENTITY_ACTIVITIES),
            issues,
            journals,
            attachments,
            pending_uploads: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ATTACHMENTS, DemoFixtureState, ISSUES};
    use crate::vos::EntityIdValue;
    use std::collections::BTreeSet;

    #[test]
    fn embedded_fixtures_parse_and_match_seed_files() {
        let state = DemoFixtureState::initial();
        assert_eq!(state.users.len(), 2);
        assert_eq!(state.issue_statuses.len(), 6);
        assert_eq!(state.priorities.len(), 4);
        assert_eq!(state.projects.len(), 2);
        assert_eq!(state.trackers.len(), 3);
        assert_eq!(state.target_versions.len(), 3);
        assert_eq!(state.categories.len(), 3);
        assert_eq!(state.time_entity_activities.len(), 3);
        let issue_ids: BTreeSet<_> = state.issues.keys().map(|id| id.get()).collect();
        assert_eq!(issue_ids, BTreeSet::from([1, 2, 3, 4]));
        for (issue_id, expected_ids) in [
            (1, BTreeSet::<u16>::new()),
            (2, BTreeSet::<u16>::new()),
            (3, BTreeSet::from([1u16, 2, 3])),
            (4, BTreeSet::<u16>::new()),
        ] {
            let journals = &state.journals[&crate::vos::IssueId::new(issue_id)];
            assert_eq!(
                journals
                    .iter()
                    .map(|journal| journal.id.get())
                    .collect::<BTreeSet<_>>(),
                expected_ids
            );
        }
        let embedded_issues: BTreeSet<_> = ISSUES.iter().map(|(id, _, _)| id.get()).collect();
        let files = |directory: &str| {
            std::fs::read_dir(directory)
                .unwrap()
                .map(|entry| {
                    entry
                        .unwrap()
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .parse::<u16>()
                        .unwrap()
                })
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(embedded_issues, files("datas/issues"));
        let embedded_journals: BTreeSet<_> = ISSUES
            .iter()
            .flat_map(|(_, _, journals)| journals.iter().map(|(id, _)| id.get()))
            .collect();
        assert_eq!(embedded_journals, files("datas/journals"));
        let embedded_attachments: BTreeSet<_> =
            ATTACHMENTS.iter().map(|(id, _, _, _)| id.get()).collect();
        assert_eq!(embedded_attachments, files("datas/attachments"));
        let issue_4_attachments = &state.attachments[&crate::vos::IssueId::new(4)];
        assert_eq!(
            issue_4_attachments
                .iter()
                .map(|attachment| attachment.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["sample.png"]
        );
    }
}
