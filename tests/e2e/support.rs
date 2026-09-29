use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use testty::session::{PtySession, PtySessionBuilder};

const BASE_URL_ENV: &str = "REDMINE_TUI_TEST_BASE_URL";
const PROJECT_NAME_ENV: &str = "REDMINE_TUI_TEST_PROJECT_NAME";
const COMPOSE_FILE_ENV: &str = "REDMINE_TUI_COMPOSE_FILE";
const API_KEY: &str = "0123456789abcdef0123456789abcdef01234567";
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
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
    spawn_app_with_api_key(Some(API_KEY))
}

/// `None`では親プロセスの`REDMINE_API_KEY`も引き継がないよう、`env -u`で外して起動する。
pub fn spawn_app_with_api_key(api_key: Option<&str>) -> PtySession {
    let app = env!("CARGO_BIN_EXE_redmine-tui-draft");
    let args = match api_key {
        Some(_) => vec![app],
        None => vec!["-u", "REDMINE_API_KEY", app],
    };
    let builder = PtySessionBuilder::new("env")
        .args(args)
        .size(TERMINAL_WIDTH, TERMINAL_HEIGHT)
        .env("REDMINE_URL", required_env(BASE_URL_ENV))
        .workdir(repo_root());
    let builder = match api_key {
        Some(api_key) => builder.env("REDMINE_API_KEY", api_key),
        None => builder,
    };
    builder
        .spawn()
        .expect("failed to start redmine-tui in a PTY")
}

/// 文字列の出現では判定できない状態（表示が消えたことなど）を、画面を取り直しながら待つ。
pub fn wait_until(
    session: &mut PtySession,
    description: &str,
    predicate: impl Fn(&str) -> bool,
) -> String {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        session.drain_output(POLL_INTERVAL);
        let frame = session.capture_frame().all_text();
        if predicate(&frame) {
            return frame;
        }
        if Instant::now() >= deadline {
            panic!("{description} did not happen\nterminal frame:\n{frame}");
        }
    }
}

pub fn press_keys(session: &mut PtySession, keys: &[&str]) {
    for key in keys {
        session
            .press_key(key)
            .unwrap_or_else(|error| panic!("failed to press {key}: {error}"));
    }
}

/// 終了コードの成否を返す。
pub fn wait_for_exit(session: &mut PtySession) -> bool {
    session.wait_for_exit(WAIT_TIMEOUT).unwrap_or_else(|error| {
        panic!(
            "redmine-tui did not exit: {error}\nterminal frame:\n{}",
            session.capture_frame().all_text()
        )
    })
}

/// Given・Thenで使うRedmine APIを、テスト用のAPI keyで呼ぶ。
pub fn redmine_api(
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> reqwest::StatusCode {
    let url = format!("{}{path}", required_env(BASE_URL_ENV));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
        .block_on(async {
            let request = reqwest::Client::new()
                .request(method, url)
                .header("X-Redmine-API-Key", API_KEY);
            let request = match body {
                Some(body) => request.json(&body),
                None => request,
            };
            request
                .send()
                .await
                .expect("failed to send Redmine API request")
                .status()
        })
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

/// Issue選択popupの見出し。popupが閉じたことは、これが画面から消えたことで判定する。
pub const POPUP_HEADER: &str = "PROJECTS";

pub fn wait_for_popup_to_close(session: &mut PtySession) -> String {
    wait_until(session, "closing the issue select popup", |frame| {
        !frame.contains(POPUP_HEADER)
    })
}

/// 起動直後のIssue選択popupから、一覧の`position`番目（0始まり）のIssueを開く。
///
/// seedの一覧はID降順（3, 2, 1）で並ぶ。
pub fn open_issue_from_initial_popup(session: &mut PtySession, position: usize) -> String {
    wait_for_text(session, "issue2");
    session.press_key("l").expect("failed to press l");
    for _ in 0..position {
        session.press_key("j").expect("failed to press j");
    }
    session.press_key("Enter").expect("failed to press Enter");
    wait_for_popup_to_close(session)
}

/// 詳細画面で`label`の直後に表示されている値を返す。
///
/// propertyは2列で表示されるため、同じ行の次の項目との区切り（2つ以上の空白）までを値とする。
pub fn property_value(frame: &str, label: &str) -> String {
    let line = frame
        .lines()
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("{label} is not displayed\nterminal frame:\n{frame}"));
    let rest = line[line.find(label).unwrap() + label.len()..].trim_start();
    rest.split("  ")
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}
