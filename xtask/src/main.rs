use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use yaml_rust::{Yaml, YamlLoader};

const ACTIVITY_ID_OFFSET: u16 = 10_000;
const REDMINE_TUI_TEST_API_USER_ID: u16 = 1;
const REDMINE_TUI_TEST_API_KEY: &str = "0123456789abcdef0123456789abcdef01234567";
const REDMINE_CLIENT_INTEGRATION_TESTS: &[&str] = &[
    "clients::redmine::default::tests::get_categories::get_categories_contract_against_redmine_container",
    "clients::redmine::default::tests::get_issue::get_issue_contract_against_redmine_container",
    "clients::redmine::default::tests::get_projects::get_projects_contract_against_redmine_container",
    "clients::redmine::default::tests::get_static_lists::get_issue_statuses_contract_against_redmine_container",
    "clients::redmine::default::tests::get_static_lists::get_priorities_contract_against_redmine_container",
    "clients::redmine::default::tests::get_static_lists::get_time_entity_activities_contract_against_redmine_container",
    "clients::redmine::default::tests::get_static_lists::get_trackers_contract_against_redmine_container",
    "clients::redmine::default::tests::get_target_versions::get_target_versions_contract_against_redmine_container",
    "clients::redmine::default::tests::get_users::get_users_contract_against_redmine_container",
];

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };

    match command.as_str() {
        "seed-redmine" => seed_redmine(args.collect()),
        "test-redmine-client" => test_redmine_client(args.collect()),
        "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn seed_redmine(args: Vec<String>) -> Result<(), String> {
    let mut dry_run = false;
    let mut datas_dir = PathBuf::from("datas");
    let mut reset_sql_path = PathBuf::from("docker/redmine/fresh_test_data.sql");
    let mut compose_file = PathBuf::from("compose.redmine.yml");
    let mut project_name = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--datas-dir" => {
                datas_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--datas-dir requires a path".to_string())?,
                );
            }
            "--reset-sql" => {
                reset_sql_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--reset-sql requires a path".to_string())?,
                );
            }
            "--compose-file" => {
                compose_file = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--compose-file requires a path".to_string())?,
                );
            }
            "--project-name" => {
                project_name = Some(args.next().ok_or_else(|| {
                    "--project-name requires a docker compose project name".to_string()
                })?);
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other => {
                return Err(format!(
                    "unknown seed-redmine option: {other}\n\n{}",
                    usage()
                ));
            }
        }
    }

    let sql = load_seed_data_sql(&datas_dir, &reset_sql_path)?;
    if dry_run {
        print!("{sql}");
        return Ok(());
    }

    run_mysql(&compose_file, project_name.as_deref(), &sql)
}

fn test_redmine_client(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }
    if let Some(arg) = args.first() {
        return Err(format!(
            "unknown test-redmine-client option: {arg}\n\n{}",
            usage()
        ));
    }

    for test_filter in REDMINE_CLIENT_INTEGRATION_TESTS {
        let status = Command::new("cargo")
            .arg("test")
            .arg(test_filter)
            .arg("--")
            .arg("--ignored")
            .arg("--test-threads=1")
            .status()
            .map_err(|err| {
                format!("failed to start redmine client integration test {test_filter}: {err}")
            })?;

        if !status.success() {
            return Err(format!(
                "redmine client integration test {test_filter} failed with status: {status}"
            ));
        }
    }

    Ok(())
}

fn usage() -> String {
    [
        "usage:",
        "  cargo xtask seed-redmine [--dry-run] [--datas-dir datas] [--reset-sql docker/redmine/fresh_test_data.sql] [--compose-file compose.redmine.yml] [--project-name NAME]",
        "  cargo xtask test-redmine-client",
    ]
    .join("\n")
}

fn run_mysql(compose_file: &Path, project_name: Option<&str>, sql: &str) -> Result<(), String> {
    let mut command = Command::new("docker");
    command.arg("compose").arg("-f").arg(compose_file);
    if let Some(project_name) = project_name {
        command.arg("-p").arg(project_name);
    }

    let mut child = command
        .arg("exec")
        .arg("-T")
        .arg("db")
        .arg("sh")
        .arg("-lc")
        .arg(mysql_shell_command())
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to start docker compose mysql command: {err}"))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "failed to open mysql stdin".to_string())?
        .write_all(sql.as_bytes())
        .map_err(|err| format!("failed to write seed SQL to mysql stdin: {err}"))?;

    let status = child
        .wait()
        .map_err(|err| format!("failed to wait for mysql command: {err}"))?;
    if !status.success() {
        return Err(format!("mysql seed command failed with status: {status}"));
    }

    Ok(())
}

fn mysql_shell_command() -> &'static str {
    "mysql --default-character-set=utf8mb4 -u\"$MYSQL_USER\" -p\"$MYSQL_PASSWORD\" \"$MYSQL_DATABASE\""
}

fn load_seed_data_sql(datas_dir: &Path, reset_sql_path: &Path) -> Result<String, String> {
    let seed = SeedData::load(datas_dir)?;
    let mut sql = fs::read_to_string(reset_sql_path)
        .map_err(|err| format!("failed to read {}: {err}", reset_sql_path.display()))?;
    if !sql.ends_with('\n') {
        sql.push('\n');
    }
    sql.push('\n');
    sql.push_str(&seed.to_sql());
    Ok(sql)
}

#[derive(Debug)]
struct NamedRecord {
    id: u16,
    name: String,
}

#[derive(Debug)]
struct IssueStatusRecord {
    id: u16,
    name: String,
    is_closed: bool,
}

#[derive(Debug)]
struct ActivityRecord {
    id: u16,
    name: String,
    is_default: bool,
}

#[derive(Debug)]
struct IssueRecord {
    id: u16,
    subject: String,
    author_id: u16,
    created_on: String,
    updated_on: String,
    project_id: u16,
    tracker_id: u16,
    status_id: u16,
    priority_id: u16,
    assigned_to_id: Option<u16>,
    target_version_id: Option<u16>,
    start_date: Option<String>,
    due_date: Option<String>,
    done_ratio: u16,
    estimated_hours: Option<f64>,
    category_id: Option<u16>,
    description: String,
    child_ids: Vec<u16>,
    journal_ids: Vec<u16>,
}

#[derive(Debug)]
struct JournalRecord {
    id: u16,
    user: String,
    updated_on: String,
    notes: String,
    details: Vec<JournalDetailRecord>,
}

#[derive(Debug)]
struct JournalDetailRecord {
    detail_type: String,
    name: String,
    old: Option<String>,
    new: Option<String>,
}

#[derive(Debug)]
struct SeedData {
    projects: Vec<NamedRecord>,
    users: Vec<NamedRecord>,
    trackers: Vec<NamedRecord>,
    statuses: Vec<IssueStatusRecord>,
    priorities: Vec<NamedRecord>,
    versions: Vec<NamedRecord>,
    categories: Vec<NamedRecord>,
    activities: Vec<ActivityRecord>,
    issues: Vec<IssueRecord>,
    journals: BTreeMap<u16, JournalRecord>,
}

impl SeedData {
    fn load(datas_dir: &Path) -> Result<Self, String> {
        Ok(Self {
            projects: load_named_records(datas_dir.join("projects.yml"), "projects")?,
            users: load_named_records(datas_dir.join("users.yml"), "users")?,
            trackers: load_named_records(datas_dir.join("trackers.yml"), "trackers")?,
            statuses: load_issue_statuses(datas_dir.join("issue_statuses.yml"))?,
            priorities: load_named_records(datas_dir.join("priorities.yml"), "priorities")?,
            versions: load_named_records(datas_dir.join("target_versions.yml"), "target_versions")?,
            categories: load_named_records(datas_dir.join("categories.yml"), "categories")?,
            activities: load_activities(datas_dir.join("time_entity_activities.yml"))?,
            issues: load_issues(&datas_dir.join("issues"))?,
            journals: load_journals(&datas_dir.join("journals"))?,
        })
    }

    fn to_sql(&self) -> String {
        let mut sql = String::new();
        sql.push_str("START TRANSACTION;\n\n");
        self.push_users_sql(&mut sql);
        self.push_api_access_sql(&mut sql);
        self.push_trackers_sql(&mut sql);
        self.push_issue_statuses_sql(&mut sql);
        self.push_enumerations_sql(&mut sql);
        self.push_projects_sql(&mut sql);
        self.push_project_support_sql(&mut sql);
        self.push_versions_sql(&mut sql);
        self.push_categories_sql(&mut sql);
        self.push_issues_sql(&mut sql);
        self.push_journals_sql(&mut sql);
        sql.push_str("COMMIT;\n");
        sql
    }

    fn push_users_sql(&self, sql: &mut String) {
        sql.push_str("INSERT INTO users (id, login, hashed_password, firstname, lastname, admin, status, language, auth_source_id, created_on, updated_on, type, mail_notification, salt, must_change_passwd) VALUES\n");
        push_values(
            sql,
            self.users.iter().map(|user| {
                let login = sql_string(&format!("redmine-tui-{}", user.name));
                format!(
                    "({}, {}, '', {}, 'Fixture', 0, 1, '', NULL, {}, {}, 'User', 'only_my_events', '', 0)",
                    db_user_id(user.id),
                    login,
                    sql_string(&user.name),
                    sql_datetime("2026/01/01"),
                    sql_datetime("2026/01/01")
                )
            }),
        );
        sql.push_str(";\n\n");

        sql.push_str("INSERT INTO email_addresses (user_id, address, is_default, notify, created_on, updated_on) VALUES\n");
        push_values(
            sql,
            self.users.iter().map(|user| {
                format!(
                    "({}, {}, 1, 1, {}, {})",
                    db_user_id(user.id),
                    sql_string(&format!("redmine-tui-{}@example.test", user.name)),
                    sql_datetime("2026/01/01"),
                    sql_datetime("2026/01/01")
                )
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_api_access_sql(&self, sql: &mut String) {
        sql.push_str(
            "DELETE FROM settings WHERE name IN ('rest_api_enabled', 'login_required');\n\n",
        );

        sql.push_str("INSERT INTO settings (name, value, updated_on) VALUES\n");
        push_values(
            sql,
            [
                "('rest_api_enabled', '1', CURRENT_TIMESTAMP)".to_string(),
                "('login_required', '1', CURRENT_TIMESTAMP)".to_string(),
            ],
        );
        sql.push_str(";\n\n");

        sql.push_str("DELETE FROM tokens WHERE action = 'api' AND value = ");
        sql.push_str(&sql_string(REDMINE_TUI_TEST_API_KEY));
        sql.push_str(";\n\n");

        sql.push_str(
            "INSERT INTO tokens (user_id, action, value, created_on, updated_on) VALUES\n",
        );
        push_values(
            sql,
            [format!(
                "({}, 'api', {}, {}, {})",
                REDMINE_TUI_TEST_API_USER_ID,
                sql_string(REDMINE_TUI_TEST_API_KEY),
                sql_datetime("2026/01/01"),
                sql_datetime("2026/01/01")
            )],
        );
        sql.push_str(";\n\n");
    }

    fn push_trackers_sql(&self, sql: &mut String) {
        sql.push_str("INSERT INTO trackers (id, name, description, position, is_in_roadmap, fields_bits, default_status_id) VALUES\n");
        push_values(
            sql,
            self.trackers.iter().enumerate().map(|(index, tracker)| {
                format!(
                    "({}, {}, '', {}, 1, 0, {})",
                    tracker.id,
                    sql_string(&tracker.name),
                    index + 1,
                    self.statuses.first().map(|status| status.id).unwrap_or(1)
                )
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_issue_statuses_sql(&self, sql: &mut String) {
        sql.push_str("INSERT INTO issue_statuses (id, name, is_closed, position, default_done_ratio) VALUES\n");
        push_values(
            sql,
            self.statuses.iter().enumerate().map(|(index, status)| {
                format!(
                    "({}, {}, {}, {}, NULL)",
                    status.id,
                    sql_string(&status.name),
                    bool_int(status.is_closed),
                    index + 1
                )
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_enumerations_sql(&self, sql: &mut String) {
        sql.push_str("INSERT INTO enumerations (id, name, position, is_default, type, active, project_id, parent_id, position_name) VALUES\n");
        let priority_values = self.priorities.iter().enumerate().map(|(index, priority)| {
            format!(
                "({}, {}, {}, {}, 'IssuePriority', 1, NULL, NULL, NULL)",
                priority.id,
                sql_string(&priority.name),
                index + 1,
                bool_int(index == 0)
            )
        });
        let activity_values = self.activities.iter().enumerate().map(|(index, activity)| {
            format!(
                "({}, {}, {}, {}, 'TimeEntryActivity', 1, NULL, NULL, NULL)",
                db_activity_id(activity.id),
                sql_string(&activity.name),
                index + 1,
                bool_int(activity.is_default)
            )
        });
        push_values(sql, priority_values.chain(activity_values));
        sql.push_str(";\n\n");
    }

    fn push_projects_sql(&self, sql: &mut String) {
        sql.push_str("INSERT INTO projects (id, name, description, homepage, is_public, parent_id, created_on, updated_on, identifier, status, lft, rgt, inherit_members) VALUES\n");
        push_values(
            sql,
            self.projects.iter().enumerate().map(|(index, project)| {
                format!(
                    "({}, {}, 'Seeded from redmine-tui datas fixtures.', '', 1, NULL, {}, {}, {}, 1, {}, {}, 0)",
                    project.id,
                    sql_string(&project.name),
                    sql_datetime("2026/01/01"),
                    sql_datetime("2026/01/01"),
                    sql_string(&project_identifier(&project.name)),
                    index * 2 + 1,
                    index * 2 + 2
                )
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_project_support_sql(&self, sql: &mut String) {
        sql.push_str("INSERT INTO projects_trackers (project_id, tracker_id) VALUES\n");
        push_values(
            sql,
            self.projects.iter().flat_map(|project| {
                self.trackers
                    .iter()
                    .map(move |tracker| format!("({}, {})", project.id, tracker.id))
            }),
        );
        sql.push_str(";\n\n");

        sql.push_str("INSERT INTO enabled_modules (project_id, name) VALUES\n");
        push_values(
            sql,
            self.projects.iter().flat_map(|project| {
                ["issue_tracking", "time_tracking"]
                    .into_iter()
                    .map(move |name| format!("({}, {})", project.id, sql_string(name)))
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_versions_sql(&self, sql: &mut String) {
        let Some(project) = self.projects.first() else {
            return;
        };
        sql.push_str("INSERT INTO versions (id, project_id, name, description, effective_date, created_on, updated_on, wiki_page_title, status, sharing) VALUES\n");
        push_values(
            sql,
            self.versions.iter().map(|version| {
                format!(
                    "({}, {}, {}, '', NULL, {}, {}, NULL, 'open', 'none')",
                    version.id,
                    project.id,
                    sql_string(&version.name),
                    sql_datetime("2026/01/01"),
                    sql_datetime("2026/01/01")
                )
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_categories_sql(&self, sql: &mut String) {
        let Some(project) = self.projects.first() else {
            return;
        };
        sql.push_str(
            "INSERT INTO issue_categories (id, project_id, name, assigned_to_id) VALUES\n",
        );
        push_values(
            sql,
            self.categories.iter().map(|category| {
                format!(
                    "({}, {}, {}, NULL)",
                    category.id,
                    project.id,
                    sql_string(&category.name)
                )
            }),
        );
        sql.push_str(";\n\n");
    }

    fn push_issues_sql(&self, sql: &mut String) {
        let closed_status_ids = self
            .statuses
            .iter()
            .filter(|status| status.is_closed)
            .map(|status| status.id)
            .collect::<BTreeSet<_>>();

        sql.push_str("INSERT INTO issues (id, tracker_id, project_id, subject, description, due_date, category_id, status_id, assigned_to_id, priority_id, fixed_version_id, author_id, lock_version, created_on, updated_on, start_date, done_ratio, estimated_hours, parent_id, root_id, lft, rgt, is_private, closed_on) VALUES\n");
        push_values(
            sql,
            self.issues.iter().map(|issue| {
                let closed_on = if closed_status_ids.contains(&issue.status_id) {
                    sql_datetime(&issue.updated_on)
                } else {
                    "NULL".to_string()
                };
                format!(
                    "({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 0, {}, {}, {}, {}, {}, NULL, {}, 1, 2, 0, {})",
                    issue.id,
                    issue.tracker_id,
                    issue.project_id,
                    sql_string(&issue.subject),
                    sql_string(&issue.description),
                    sql_date_option(issue.due_date.as_deref()),
                    sql_option_u16(issue.category_id),
                    issue.status_id,
                    sql_option_u16(issue.assigned_to_id),
                    issue.priority_id,
                    sql_option_u16(issue.target_version_id),
                    db_user_id(issue.author_id),
                    sql_datetime(&issue.created_on),
                    sql_datetime(&issue.updated_on),
                    sql_date_option(issue.start_date.as_deref()),
                    issue.done_ratio,
                    sql_option_f64(issue.estimated_hours),
                    issue.id,
                    closed_on
                )
            }),
        );
        sql.push_str(";\n\n");

        for update in issue_tree_updates(&self.issues) {
            sql.push_str(&update);
            sql.push('\n');
        }
        sql.push('\n');
    }

    fn push_journals_sql(&self, sql: &mut String) {
        let user_ids_by_name = self
            .users
            .iter()
            .map(|user| (user.name.as_str(), db_user_id(user.id)))
            .collect::<BTreeMap<_, _>>();
        let issue_ids_by_journal_id = self
            .issues
            .iter()
            .flat_map(|issue| {
                issue
                    .journal_ids
                    .iter()
                    .map(move |journal_id| (*journal_id, issue.id))
            })
            .collect::<BTreeMap<_, _>>();

        let journals = self
            .journals
            .values()
            .filter_map(|journal| {
                let issue_id = issue_ids_by_journal_id.get(&journal.id)?;
                let user_id = user_ids_by_name
                    .get(journal.user.as_str())
                    .copied()
                    .unwrap_or(1);
                Some((journal, *issue_id, user_id))
            })
            .collect::<Vec<_>>();

        if journals.is_empty() {
            return;
        }

        sql.push_str("INSERT INTO journals (id, journalized_id, journalized_type, user_id, notes, created_on, private_notes) VALUES\n");
        push_values(
            sql,
            journals.iter().map(|(journal, issue_id, user_id)| {
                format!(
                    "({}, {}, 'Issue', {}, {}, {}, 0)",
                    journal.id,
                    issue_id,
                    user_id,
                    sql_string(&journal.notes),
                    sql_datetime(&journal.updated_on)
                )
            }),
        );
        sql.push_str(";\n\n");

        let detail_values = journals
            .iter()
            .flat_map(|(journal, _, _)| {
                journal.details.iter().map(move |detail| {
                    format!(
                        "({}, {}, {}, {}, {})",
                        journal.id,
                        sql_string(&detail.detail_type),
                        sql_string(&detail.name),
                        sql_string_option(detail.old.as_deref()),
                        sql_string_option(detail.new.as_deref())
                    )
                })
            })
            .collect::<Vec<_>>();

        if !detail_values.is_empty() {
            sql.push_str("INSERT INTO journal_details (journal_id, property, prop_key, old_value, value) VALUES\n");
            push_values(sql, detail_values);
            sql.push_str(";\n\n");
        }
    }
}

fn load_named_records(path: PathBuf, key: &str) -> Result<Vec<NamedRecord>, String> {
    let yaml = read_yaml(&path)?;
    yaml[key]
        .as_vec()
        .ok_or_else(|| format!("{} has no {key} array", path.display()))?
        .iter()
        .map(|entry| {
            Ok(NamedRecord {
                id: as_u16(entry, "id")?,
                name: as_string(entry, "name")?,
            })
        })
        .collect()
}

fn load_issue_statuses(path: PathBuf) -> Result<Vec<IssueStatusRecord>, String> {
    let yaml = read_yaml(&path)?;
    yaml["issue_statuses"]
        .as_vec()
        .ok_or_else(|| format!("{} has no issue_statuses array", path.display()))?
        .iter()
        .map(|entry| {
            Ok(IssueStatusRecord {
                id: as_u16(entry, "id")?,
                name: as_string(entry, "name")?,
                is_closed: as_bool(entry, "is_closed")?,
            })
        })
        .collect()
}

fn load_activities(path: PathBuf) -> Result<Vec<ActivityRecord>, String> {
    let yaml = read_yaml(&path)?;
    yaml["time_entity_activities"]
        .as_vec()
        .ok_or_else(|| format!("{} has no time_entity_activities array", path.display()))?
        .iter()
        .map(|entry| {
            Ok(ActivityRecord {
                id: as_u16(entry, "id")?,
                name: as_string(entry, "name")?,
                is_default: as_bool(entry, "is_default")?,
            })
        })
        .collect()
}

fn load_issues(dir: &Path) -> Result<Vec<IssueRecord>, String> {
    let mut paths = numbered_yaml_paths(dir)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let yaml = read_yaml(&path)?;
            Ok(IssueRecord {
                id: as_u16(&yaml, "id")?,
                subject: as_string(&yaml, "subject")?,
                author_id: as_u16(&yaml, "author_id")?,
                created_on: as_string(&yaml, "created_on")?,
                updated_on: as_string(&yaml, "updated_on")?,
                project_id: as_u16(&yaml, "project_id")?,
                tracker_id: as_u16(&yaml, "tracker_id")?,
                status_id: as_u16(&yaml, "status_id")?,
                priority_id: as_u16(&yaml, "priority_id")?,
                assigned_to_id: as_u16_option(&yaml, "assigned_to_id")?,
                target_version_id: as_u16_option(&yaml, "target_version_id")?,
                start_date: as_string_option(&yaml, "start_date")?,
                due_date: as_string_option(&yaml, "due_date")?,
                done_ratio: as_u16(&yaml, "done_ratio")?,
                estimated_hours: as_f64_option(&yaml, "estimated_hours")?,
                category_id: as_u16_option(&yaml, "category_id")?,
                description: as_string(&yaml, "description")?,
                child_ids: as_u16_array(&yaml, "child_ids")?,
                journal_ids: as_u16_array(&yaml, "journal_ids")?,
            })
        })
        .collect()
}

fn load_journals(dir: &Path) -> Result<BTreeMap<u16, JournalRecord>, String> {
    let mut paths = numbered_yaml_paths(dir)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let yaml = read_yaml(&path)?;
            let id = as_u16(&yaml, "id")?;
            Ok((
                id,
                JournalRecord {
                    id,
                    user: as_string(&yaml, "user")?,
                    updated_on: as_string(&yaml, "updated_on")?,
                    notes: as_string_option(&yaml, "notes")?.unwrap_or_default(),
                    details: yaml["details"]
                        .as_vec()
                        .ok_or_else(|| format!("{} has no details array", path.display()))?
                        .iter()
                        .map(|detail| {
                            Ok(JournalDetailRecord {
                                detail_type: as_string(detail, "type")?,
                                name: as_string(detail, "name")?,
                                old: as_string_option(detail, "old")?,
                                new: as_string_option(detail, "new")?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                },
            ))
        })
        .collect()
}

fn numbered_yaml_paths(dir: &Path) -> Result<Vec<PathBuf>, String> {
    fs::read_dir(dir)
        .map_err(|err| format!("failed to read {}: {err}", dir.display()))?
        .map(|entry| {
            let path = entry
                .map_err(|err| format!("failed to read entry in {}: {err}", dir.display()))?
                .path();
            let id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u16>().ok())
                .ok_or_else(|| format!("expected numbered yaml file in {}", path.display()))?;
            Ok((id, path))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()
        .map(|paths| paths.into_values().collect())
}

fn read_yaml(path: &Path) -> Result<Yaml, String> {
    let yaml = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let docs = YamlLoader::load_from_str(&yaml)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    docs.into_iter()
        .next()
        .ok_or_else(|| format!("{} is empty", path.display()))
}

fn as_u16(yaml: &Yaml, key: &str) -> Result<u16, String> {
    yaml[key]
        .as_i64()
        .ok_or_else(|| format!("missing integer field: {key}"))?
        .try_into()
        .map_err(|_| format!("integer field out of range for u16: {key}"))
}

fn as_u16_option(yaml: &Yaml, key: &str) -> Result<Option<u16>, String> {
    yaml[key]
        .as_i64()
        .map(|value| {
            value
                .try_into()
                .map_err(|_| format!("integer field out of range for u16: {key}"))
        })
        .transpose()
}

fn as_f64_option(yaml: &Yaml, key: &str) -> Result<Option<f64>, String> {
    if yaml[key].is_badvalue() {
        return Ok(None);
    }
    yaml[key]
        .as_f64()
        .or_else(|| yaml[key].as_i64().map(|value| value as f64))
        .map(Some)
        .ok_or_else(|| format!("field is not a number: {key}"))
}

fn as_string(yaml: &Yaml, key: &str) -> Result<String, String> {
    yaml[key]
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing string field: {key}"))
}

fn as_string_option(yaml: &Yaml, key: &str) -> Result<Option<String>, String> {
    if yaml[key].is_badvalue() {
        return Ok(None);
    }
    Ok(yaml[key].as_str().map(ToString::to_string))
}

fn as_bool(yaml: &Yaml, key: &str) -> Result<bool, String> {
    yaml[key]
        .as_bool()
        .ok_or_else(|| format!("missing boolean field: {key}"))
}

fn as_u16_array(yaml: &Yaml, key: &str) -> Result<Vec<u16>, String> {
    if yaml[key].is_badvalue() {
        return Ok(Vec::new());
    }
    let Some(values) = yaml[key].as_vec() else {
        return Ok(Vec::new());
    };
    values
        .iter()
        .map(|value| {
            value
                .as_i64()
                .ok_or_else(|| format!("{key} contains a non-integer value"))?
                .try_into()
                .map_err(|_| format!("{key} contains a value out of range for u16"))
        })
        .collect()
}

fn issue_tree_updates(issues: &[IssueRecord]) -> Vec<String> {
    let child_to_parent = issues
        .iter()
        .flat_map(|issue| {
            issue
                .child_ids
                .iter()
                .map(move |child_id| (*child_id, issue.id))
        })
        .collect::<BTreeMap<_, _>>();
    let children_by_parent = issues
        .iter()
        .map(|issue| {
            let mut children = issue.child_ids.clone();
            children.sort();
            (issue.id, children)
        })
        .collect::<BTreeMap<_, _>>();
    let ids = issues.iter().map(|issue| issue.id).collect::<BTreeSet<_>>();
    let mut roots = ids
        .iter()
        .copied()
        .filter(|id| !child_to_parent.contains_key(id))
        .collect::<Vec<_>>();
    roots.sort();

    let mut updates = Vec::new();
    for root_id in roots {
        let mut cursor = 1;
        assign_nested_set(
            root_id,
            None,
            root_id,
            &children_by_parent,
            &mut cursor,
            &mut updates,
        );
    }
    updates
}

fn assign_nested_set(
    issue_id: u16,
    parent_id: Option<u16>,
    root_id: u16,
    children_by_parent: &BTreeMap<u16, Vec<u16>>,
    cursor: &mut i32,
    updates: &mut Vec<String>,
) {
    let lft = *cursor;
    *cursor += 1;
    if let Some(children) = children_by_parent.get(&issue_id) {
        for child_id in children {
            assign_nested_set(
                *child_id,
                Some(issue_id),
                root_id,
                children_by_parent,
                cursor,
                updates,
            );
        }
    }
    let rgt = *cursor;
    *cursor += 1;

    updates.push(format!(
        "UPDATE issues SET parent_id = {}, root_id = {}, lft = {}, rgt = {} WHERE id = {};",
        sql_option_u16(parent_id),
        root_id,
        lft,
        rgt,
        issue_id
    ));
}

fn push_values(sql: &mut String, values: impl IntoIterator<Item = String>) {
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            sql.push_str(",\n");
        }
        sql.push_str("  ");
        sql.push_str(&value);
    }
}

fn sql_string(value: &str) -> String {
    let mut quoted = String::from("'");
    for ch in value.chars() {
        match ch {
            '\'' => quoted.push_str("\\'"),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\0' => quoted.push_str("\\0"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('\'');
    quoted
}

fn sql_string_option(value: Option<&str>) -> String {
    value.map(sql_string).unwrap_or_else(|| "NULL".to_string())
}

fn sql_option_u16(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn sql_option_f64(value: Option<f64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn sql_date_option(value: Option<&str>) -> String {
    value
        .map(|value| sql_string(&value.replace('/', "-")))
        .unwrap_or_else(|| "NULL".to_string())
}

fn sql_datetime(value: &str) -> String {
    sql_string(&format!("{} 00:00:00", value.replace('/', "-")))
}

fn bool_int(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

fn db_activity_id(activity_id: u16) -> u16 {
    ACTIVITY_ID_OFFSET + activity_id
}

fn db_user_id(user_id: u16) -> u16 {
    user_id
}

fn project_identifier(name: &str) -> String {
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_dash = false;
        } else if !previous_dash && !out.is_empty() {
            out.push('-');
            previous_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "redmine-tui-project".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_path(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
    }

    #[test]
    fn sql_string_escapes_quotes_backslashes_and_newlines() {
        assert_eq!(
            sql_string("it's \\ fine\nnext"),
            "'it\\'s \\\\ fine\\nnext'"
        );
    }

    #[test]
    fn seed_sql_uses_datas_issue_ids_and_relationships() {
        let sql = load_seed_data_sql(
            &repo_path("datas"),
            &repo_path("docker/redmine/fresh_test_data.sql"),
        )
        .expect("seed SQL should be generated from repository fixtures");

        assert!(sql.contains("INSERT INTO issues"));
        assert!(sql.contains("(1, 1, 1, 'issue1'"));
        assert!(sql.contains("(3, 3, 1, 'issue1(長"));
        assert!(sql.contains(
            "UPDATE issues SET parent_id = 3, root_id = 3, lft = 2, rgt = 3 WHERE id = 1;"
        ));
        assert!(sql.contains(
            "UPDATE issues SET parent_id = 3, root_id = 3, lft = 4, rgt = 5 WHERE id = 2;"
        ));
    }

    #[test]
    fn seed_sql_includes_journals_and_details_from_datas() {
        let sql = load_seed_data_sql(
            &repo_path("datas"),
            &repo_path("docker/redmine/fresh_test_data.sql"),
        )
        .expect("seed SQL should be generated from repository fixtures");

        assert!(sql.contains("INSERT INTO journals"));
        assert!(sql.contains("(1, 3, 'Issue', 1001, ''"));
        assert!(sql.contains("INSERT INTO journal_details"));
        assert!(sql.contains("'attr', 'status_id', '新規(new)', '割り当て(assigned)'"));
        assert!(sql.contains("'attr', 'due_date', '2026/02/16', '2026/02/17'"));
    }

    #[test]
    fn seed_sql_uses_datas_user_ids_as_redmine_user_ids() {
        let sql = load_seed_data_sql(
            &repo_path("datas"),
            &repo_path("docker/redmine/fresh_test_data.sql"),
        )
        .expect("seed SQL should be generated from repository fixtures");

        assert!(sql.contains("(1001, 'redmine-tui-user1'"));
        assert!(sql.contains("(1002, 'redmine-tui-user2'"));
        assert!(sql.contains(", 1001, 0, '2026-01-01 00:00:00'"));
        assert!(!sql.contains("(2001, 'redmine-tui-user1'"));
    }

    #[test]
    fn seed_sql_enables_rest_api_and_inserts_admin_api_token() {
        let sql = load_seed_data_sql(
            &repo_path("datas"),
            &repo_path("docker/redmine/fresh_test_data.sql"),
        )
        .expect("seed SQL should be generated from repository fixtures");

        assert!(sql.contains("('rest_api_enabled', '1'"));
        assert!(sql.contains("('login_required', '1'"));
        assert!(sql.contains("('rest_api_enabled', '1', CURRENT_TIMESTAMP)"));
        assert!(sql.contains("('login_required', '1', CURRENT_TIMESTAMP)"));
        assert!(sql.contains(
            "DELETE FROM settings WHERE name IN ('rest_api_enabled', 'login_required');"
        ));
        assert!(!sql.contains("ON DUPLICATE KEY UPDATE"));
        assert!(sql.contains(
            "DELETE FROM tokens WHERE action = 'api' AND value = '0123456789abcdef0123456789abcdef01234567';"
        ));
        assert!(sql.contains("(1, 'api', '0123456789abcdef0123456789abcdef01234567'"));
        assert!(sql.contains("'0123456789abcdef0123456789abcdef01234567'"));
    }

    #[test]
    fn seed_sql_and_mysql_command_force_utf8mb4() {
        let sql = load_seed_data_sql(
            &repo_path("datas"),
            &repo_path("docker/redmine/fresh_test_data.sql"),
        )
        .expect("seed SQL should be generated from repository fixtures");

        assert!(sql.starts_with("SET NAMES utf8mb4;\n"));
        assert!(mysql_shell_command().contains("--default-character-set=utf8mb4"));
    }
}
