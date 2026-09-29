use std::path::PathBuf;
use std::process::Command;

mod build_pages;
mod redmine_container;
mod seed_redmine;

const CONTAINER_TEST_FEATURE: &str = "container-tests";
const CONTAINER_TEST_MODULE: &str = "clients::redmine::default::tests::container::";

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

    let container =
        redmine_container::RedmineContainer::start(PathBuf::from("compose.redmine.yml"))?;
    let mut command = Command::new("cargo");
    command
        .arg("test")
        .arg("--features")
        .arg(CONTAINER_TEST_FEATURE)
        .arg(CONTAINER_TEST_MODULE)
        .arg("--")
        .arg("--test-threads=1");
    container.apply_env(&mut command);
    let status = command
        .status()
        .map_err(|err| format!("failed to start redmine client integration tests: {err}"))?;

    if !status.success() {
        container.report_failure();
        return Err(format!(
            "redmine client integration tests failed with status: {status}"
        ));
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
