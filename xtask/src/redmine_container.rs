//! テスト用のRedmine containerを1回の実行で共有するため、起動から破棄までを管理する。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::seed_redmine::{REDMINE_TUI_TEST_API_KEY, seed_redmine};

pub(crate) const BASE_URL_ENV: &str = "REDMINE_TUI_TEST_BASE_URL";
pub(crate) const PROJECT_NAME_ENV: &str = "REDMINE_TUI_TEST_PROJECT_NAME";
pub(crate) const COMPOSE_FILE_ENV: &str = "REDMINE_TUI_COMPOSE_FILE";

const READY_RETRIES: usize = 120;
const RETRY_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct RedmineContainer {
    compose_file: PathBuf,
    project_name: String,
    port: u16,
}

impl RedmineContainer {
    pub(crate) fn start(compose_file: PathBuf) -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time is before Unix epoch: {err}"))?
            .as_nanos();
        let project_name = format!("redmine-tui-test-{}-{nanos}", std::process::id());

        let output = compose_command(&compose_file, &project_name)
            .arg("up")
            .arg("-d")
            .env("REDMINE_PORT", "0")
            .output()
            .map_err(|err| format!("failed to start docker compose up: {err}"))?;
        // upが途中で失敗しても作成済みのcontainerとvolumeを破棄できるよう、先にselfを作る。
        let mut container = Self {
            compose_file,
            project_name,
            port: 0,
        };
        expect_success("docker compose up", &output)?;

        container.port = container.redmine_port()?;
        // Redmineはmigration完了後にlistenを始めるため、応答を待ってからseedする。
        // migration中にseedが通ると、後続のmigrationが失敗してRedmineが起動しない。
        container.wait_until_ready()?;
        container.seed()?;
        container.clear_cache()?;
        Ok(container)
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// テストプロセスへ接続先と再seedに必要な情報を渡す。
    pub(crate) fn apply_env(&self, command: &mut Command) {
        command
            .env(BASE_URL_ENV, self.base_url())
            .env(PROJECT_NAME_ENV, &self.project_name)
            .env(COMPOSE_FILE_ENV, &self.compose_file);
    }

    fn redmine_port(&self) -> Result<u16, String> {
        let output = compose_command(&self.compose_file, &self.project_name)
            .arg("port")
            .arg("redmine")
            .arg("3000")
            .output()
            .map_err(|err| format!("failed to start docker compose port: {err}"))?;
        expect_success("docker compose port", &output)?;
        let address = String::from_utf8_lossy(&output.stdout);
        address
            .trim()
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
            .ok_or_else(|| format!("unexpected docker compose port output: {address}"))
    }

    fn seed(&self) -> Result<(), String> {
        seed_redmine(vec![
            "--compose-file".to_string(),
            self.compose_file.display().to_string(),
            "--project-name".to_string(),
            self.project_name.clone(),
        ])
    }

    fn clear_cache(&self) -> Result<(), String> {
        let output = compose_command(&self.compose_file, &self.project_name)
            .arg("exec")
            .arg("-T")
            .arg("redmine")
            .arg("sh")
            .arg("-lc")
            .arg("SECRET_KEY_BASE=\"$REDMINE_SECRET_KEY_BASE\" bundle exec rails runner 'Rails.cache.clear'")
            .output()
            .map_err(|err| format!("failed to start Redmine cache clear: {err}"))?;
        expect_success("Redmine cache clear", &output)
    }

    fn wait_until_ready(&self) -> Result<(), String> {
        let mut last_response = "no response".to_string();
        for _ in 0..READY_RETRIES {
            match self.get_status("/issues/1.json") {
                Ok(status) if status < 500 => return Ok(()),
                Ok(status) => last_response = format!("status {status}"),
                Err(err) => last_response = err,
            }
            sleep(RETRY_INTERVAL);
        }
        Err(format!(
            "Redmine did not become ready at {}; last response: {last_response}",
            self.base_url()
        ))
    }

    fn get_status(&self, path: &str) -> Result<u16, String> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port))
            .map_err(|err| format!("failed to connect to Redmine: {err}"))?;
        write!(
            stream,
            "GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nX-Redmine-API-Key: {REDMINE_TUI_TEST_API_KEY}\r\n\r\n"
        )
        .map_err(|err| format!("failed to send request to Redmine: {err}"))?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|err| format!("failed to read Redmine response: {err}"))?;
        response
            .split_whitespace()
            .nth(1)
            .and_then(|status| status.parse().ok())
            .ok_or_else(|| "Redmine returned a malformed status line".to_string())
    }

    fn logs(&self) -> String {
        match compose_command(&self.compose_file, &self.project_name)
            .arg("logs")
            .arg("--tail=200")
            .arg("redmine")
            .arg("db")
            .output()
        {
            Ok(output) => format!(
                "docker compose logs:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(err) => format!("failed to collect docker compose logs: {err}"),
        }
    }

    pub(crate) fn report_failure(&self) {
        eprintln!("{}", self.logs());
    }
}

impl Drop for RedmineContainer {
    fn drop(&mut self) {
        let result = compose_command(&self.compose_file, &self.project_name)
            .arg("down")
            .arg("-v")
            .output();
        match result {
            Ok(output) if output.status.success() => {}
            Ok(output) => eprintln!(
                "docker compose down failed for {}: {}",
                self.project_name,
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(err) => eprintln!(
                "failed to start docker compose down for {}: {err}",
                self.project_name
            ),
        }
    }
}

fn compose_command(compose_file: &Path, project_name: &str) -> Command {
    let mut command = Command::new("docker");
    command
        .arg("compose")
        .arg("-f")
        .arg(compose_file)
        .arg("-p")
        .arg(project_name);
    command
}

fn expect_success(name: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{name} failed with status {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    ))
}
