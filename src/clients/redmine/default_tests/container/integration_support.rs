use std::error::Error;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::clients::redmine::{DefaultRedmineClient, RedmineClientError};

pub(super) const TEST_API_KEY: &str = "0123456789abcdef0123456789abcdef01234567";

const BASE_URL_ENV: &str = "REDMINE_TUI_TEST_BASE_URL";
const PROJECT_NAME_ENV: &str = "REDMINE_TUI_TEST_PROJECT_NAME";

/// `cargo xtask test-redmine-client`が起動したcontainerへseedを入れ直してから検証する。
pub(super) fn run_contract<F, Fut>(contract: F)
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<(), Box<dyn Error>>>,
{
    super::super::block_on(async {
        run_contract_inner(contract)
            .await
            .expect("Redmine contract test should pass");
    });
}

async fn run_contract_inner<F, Fut>(contract: F) -> Result<(), Box<dyn Error>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<(), Box<dyn Error>>>,
{
    let base_url = required_env(BASE_URL_ENV)?;
    reseed_redmine()?;
    contract(base_url).await
}

pub(super) fn reseed_redmine() -> Result<(), Box<dyn Error>> {
    let repo_root = repo_root();
    let status = Command::new("cargo")
        .arg("xtask")
        .arg("seed-redmine")
        .arg("--compose-file")
        .arg(compose_file(&repo_root))
        .arg("--project-name")
        .arg(required_env(PROJECT_NAME_ENV)?)
        .current_dir(&repo_root)
        .status()?;
    if !status.success() {
        return Err(test_error(format!(
            "cargo xtask seed-redmine failed with status: {status}"
        )));
    }
    Ok(())
}

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    std::env::var(name).map_err(|_| {
        test_error(format!(
            "{name} is not set; run container tests through `cargo xtask test-redmine-client`"
        ))
    })
}

pub(super) fn authenticated_client(base_url: &str) -> DefaultRedmineClient {
    DefaultRedmineClient::new(base_url, TEST_API_KEY)
}

pub(super) fn unauthorized_client(base_url: &str) -> DefaultRedmineClient {
    DefaultRedmineClient::new(base_url, "invalid-redmine-api-key")
}

pub(super) fn not_found_client(base_url: &str) -> DefaultRedmineClient {
    DefaultRedmineClient::new(format!("{base_url}/missing"), TEST_API_KEY)
}

pub(super) async fn expect_unauthorized<T>(
    result: Result<T, RedmineClientError>,
) -> Result<(), Box<dyn Error>> {
    match result {
        Err(RedmineClientError::Unauthorized { context }) => {
            assert_eq!(context.status_code, 401);
            Ok(())
        }
        Ok(_) => Err(test_error("expected Unauthorized, got Ok")),
        Err(other) => Err(test_error(format!("expected Unauthorized, got {other:?}"))),
    }
}

pub(super) async fn expect_not_found<T>(
    result: Result<T, RedmineClientError>,
) -> Result<(), Box<dyn Error>> {
    match result {
        Err(RedmineClientError::NotFound { context }) => {
            assert_eq!(context.status_code, 404);
            Ok(())
        }
        Ok(_) => Err(test_error("expected NotFound, got Ok")),
        Err(other) => Err(test_error(format!("expected NotFound, got {other:?}"))),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn compose_file(repo_root: &Path) -> PathBuf {
    std::env::var_os("REDMINE_TUI_COMPOSE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("compose.redmine.yml"))
}

pub(super) fn test_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}
