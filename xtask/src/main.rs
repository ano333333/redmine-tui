use std::path::PathBuf;
use std::process::Command;

mod build_pages;
mod redmine_container;
mod seed_redmine;

const CONTAINER_TEST_FEATURE: &str = "container-tests";
const CONTAINER_TEST_MODULE: &str = "clients::redmine::default::tests::container::";
const E2E_TEST_FEATURE: &str = "e2e-tests";
const E2E_TEST_TARGET: &str = "e2e";

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
        "test-e2e" => test_e2e(args.collect()),
        "build-pages" => build_pages::build_pages(args.collect()),
        "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn test_redmine_client(args: Vec<String>) -> Result<(), String> {
    run_container_tests(
        "test-redmine-client",
        args,
        &["--features", CONTAINER_TEST_FEATURE, CONTAINER_TEST_MODULE],
    )
}

fn test_e2e(args: Vec<String>) -> Result<(), String> {
    // E2EはPTY上でnativeバイナリを起動するため、`CARGO_BIN_EXE_*`を使えるtest targetとして置く。
    run_container_tests(
        "test-e2e",
        args,
        &["--features", E2E_TEST_FEATURE, "--test", E2E_TEST_TARGET],
    )
}

/// Redmine containerを1つ起動し、指定したテストを直列に1回の`cargo test`で実行する。
fn run_container_tests(name: &str, args: Vec<String>, cargo_args: &[&str]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }
    if let Some(arg) = args.first() {
        return Err(format!("unknown {name} option: {arg}\n\n{}", usage()));
    }

    let container =
        redmine_container::RedmineContainer::start(PathBuf::from("compose.redmine.yml"))?;
    let mut command = Command::new("cargo");
    command
        .arg("test")
        .args(cargo_args)
        .arg("--")
        .arg("--test-threads=1");
    container.apply_env(&mut command);
    let status = command
        .status()
        .map_err(|err| format!("failed to start {name}: {err}"))?;

    if !status.success() {
        container.report_failure();
        return Err(format!("{name} failed with status: {status}"));
    }

    Ok(())
}

pub(crate) fn usage() -> String {
    [
        "usage:",
        "  cargo xtask seed-redmine [--dry-run] [--datas-dir datas] [--reset-sql docker/redmine/fresh_test_data.sql] [--compose-file compose.redmine.yml] [--project-name NAME]",
        "  cargo xtask test-redmine-client",
        "  cargo xtask test-e2e",
        "  cargo xtask build-pages --public-url URL",
    ]
    .join("\n")
}
