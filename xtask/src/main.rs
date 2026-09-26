use std::process::Command;

mod build_pages;
mod seed_redmine;

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
        "seed-redmine" => seed_redmine::seed_redmine(args.collect()),
        "test-redmine-client" => test_redmine_client(args.collect()),
        "build-pages" => build_pages::build_pages(args.collect()),
        "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(usage()),
    }
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

pub(crate) fn usage() -> String {
    [
        "usage:",
        "  cargo xtask seed-redmine [--dry-run] [--datas-dir datas] [--reset-sql docker/redmine/fresh_test_data.sql] [--compose-file compose.redmine.yml] [--project-name NAME]",
        "  cargo xtask test-redmine-client",
        "  cargo xtask build-pages --public-url URL",
    ]
    .join("\n")
}
