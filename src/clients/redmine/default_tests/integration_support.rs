use std::error::Error;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use testcontainers::compose::DockerCompose;

use crate::clients::redmine::{DefaultRedmineClient, RedmineClientError};

pub(super) const TEST_API_KEY: &str = "0123456789abcdef0123456789abcdef01234567";

pub(super) fn run_contract<F, Fut>(contract: F)
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<(), Box<dyn Error>>>,
{
    super::block_on(async {
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
    let repo_root = repo_root();
    let compose_file = compose_file(&repo_root);
    let project_name = unique_project_name();

    let mut compose = DockerCompose::with_local_client(&[compose_file.as_path()])
        .with_project_name(&project_name)
        .with_env("REDMINE_PORT", "0");
    compose.with_remove_volumes(true);
    compose.up().await?;

    let redmine = compose
        .service("redmine")
        .ok_or_else(|| test_error("redmine service should exist"))?;
    let redmine_port = redmine.get_host_port_ipv4(3000).await?;
    let base_url = format!("http://127.0.0.1:{redmine_port}");

    let contract_result = async {
        seed_redmine_with_retry(&repo_root, &compose_file, &project_name).await?;
        clear_redmine_cache(&repo_root, &compose_file, &project_name)?;
        wait_for_redmine(&base_url).await?;
        contract(base_url).await
    }
    .await;

    let contract_result = contract_result.map_err(|error| {
        test_error(format!(
            "{error}\n\n{}",
            compose_logs(&repo_root, &compose_file, &project_name)
        ))
    });
    let down_result = compose.down().await.map_err(Box::<dyn Error>::from);

    contract_result?;
    down_result?;
    Ok(())
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

fn unique_project_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after Unix epoch")
        .as_nanos();
    format!("redmine-tui-client-test-{}-{nanos}", std::process::id())
}

async fn wait_for_redmine(base_url: &str) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let health_url = format!("{base_url}/issues/1.json?include=children,journals");
    let mut last_response = "no response".to_string();

    for _ in 0..120 {
        match client
            .get(&health_url)
            .header("X-Redmine-API-Key", TEST_API_KEY)
            .send()
            .await
        {
            Ok(response) if response.status().as_u16() < 500 => return Ok(()),
            Ok(response) => {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|error| format!("<failed to read body: {error}>"));
                last_response = format!("{status}: {}", body.chars().take(500).collect::<String>());
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(error) => {
                last_response = error.to_string();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Err(test_error(format!(
        "Redmine did not become ready at {base_url}; last response: {last_response}"
    )))
}

async fn seed_redmine_with_retry(
    repo_root: &Path,
    compose_file: &Path,
    project_name: &str,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..120 {
        if seed_redmine(repo_root, compose_file, project_name)? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Err(test_error("cargo xtask seed-redmine did not succeed"))
}

fn seed_redmine(
    repo_root: &Path,
    compose_file: &Path,
    project_name: &str,
) -> Result<bool, Box<dyn Error>> {
    let status = Command::new("cargo")
        .arg("xtask")
        .arg("seed-redmine")
        .arg("--compose-file")
        .arg(compose_file)
        .arg("--project-name")
        .arg(project_name)
        .current_dir(repo_root)
        .status()?;

    Ok(status.success())
}

fn clear_redmine_cache(
    repo_root: &Path,
    compose_file: &Path,
    project_name: &str,
) -> Result<(), Box<dyn Error>> {
    let status = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg(compose_file)
        .arg("-p")
        .arg(project_name)
        .arg("exec")
        .arg("-T")
        .arg("redmine")
        .arg("sh")
        .arg("-lc")
        .arg("SECRET_KEY_BASE=\"$REDMINE_SECRET_KEY_BASE\" bundle exec rails runner 'Rails.cache.clear'")
        .current_dir(repo_root)
        .status()?;

    if !status.success() {
        return Err(test_error(format!(
            "docker compose exec redmine cache clear failed with status: {status}"
        )));
    }

    Ok(())
}

fn compose_logs(repo_root: &Path, compose_file: &Path, project_name: &str) -> String {
    match Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg(compose_file)
        .arg("-p")
        .arg(project_name)
        .arg("logs")
        .arg("--tail=200")
        .arg("redmine")
        .arg("db")
        .current_dir(repo_root)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!("docker compose logs:\n{stdout}{stderr}")
        }
        Err(error) => format!("failed to collect docker compose logs: {error}"),
    }
}

pub(super) fn test_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}
