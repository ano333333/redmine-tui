use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use testty::session::{PtySession, PtySessionBuilder};

const BASE_URL_ENV: &str = "REDMINE_TUI_TEST_BASE_URL";
const PROJECT_NAME_ENV: &str = "REDMINE_TUI_TEST_PROJECT_NAME";
const COMPOSE_FILE_ENV: &str = "REDMINE_TUI_COMPOSE_FILE";
const API_KEY: &str = "0123456789abcdef0123456789abcdef01234567";
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const TERMINAL_WIDTH: u16 = 120;
const TERMINAL_HEIGHT: u16 = 40;

/// シナリオ間で状態を共有しないよう、開始前にseedを入れ直す。
pub fn reseed_redmine() {
    let status = Command::new("cargo")
        .arg("xtask")
        .arg("seed-redmine")
        .arg("--compose-file")
        .arg(required_env(COMPOSE_FILE_ENV))
        .arg("--project-name")
        .arg(required_env(PROJECT_NAME_ENV))
        .current_dir(repo_root())
        .status()
        .expect("failed to start cargo xtask seed-redmine");
    assert!(
        status.success(),
        "cargo xtask seed-redmine failed: {status}"
    );
}

pub fn spawn_app() -> PtySession {
    PtySessionBuilder::new(env!("CARGO_BIN_EXE_redmine-tui-draft"))
        .size(TERMINAL_WIDTH, TERMINAL_HEIGHT)
        .env("REDMINE_URL", required_env(BASE_URL_ENV))
        .env("REDMINE_API_KEY", API_KEY)
        .workdir(repo_root())
        .spawn()
        .expect("failed to start redmine-tui in a PTY")
}

/// 失敗時に画面全体を出力し、どの画面で止まったかを確認できるようにする。
pub fn wait_for_text(session: &mut PtySession, text: &str) -> String {
    match session.wait_for_text(text, WAIT_TIMEOUT) {
        Ok(frame) => frame.all_text(),
        Err(error) => panic!(
            "{text:?} did not appear: {error}\nterminal frame:\n{}",
            session.capture_frame().all_text()
        ),
    }
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} is not set; run E2E scenarios through `cargo xtask test-e2e`")
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
