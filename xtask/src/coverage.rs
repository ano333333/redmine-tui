//! unit testとE2Eのcoverageを計測し、1つのレポートにまとめる。

use std::path::Path;
use std::process::Command;

pub(crate) fn coverage(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", crate::usage());
        return Ok(());
    }
    let report_args = if args.is_empty() {
        vec!["--html".to_string()]
    } else {
        args
    };

    let envs = coverage_envs()?;
    run_cargo(
        "cargo llvm-cov clean",
        &["llvm-cov", "clean", "--workspace"],
        &envs,
    )?;
    run_cargo("cargo test", &["test"], &envs)?;
    crate::run_cargo_test_with_container("test-e2e", crate::E2E_TEST_ARGS, &envs)?;

    let mut report = vec!["llvm-cov", "report"];
    report.extend(report_args.iter().map(String::as_str));
    run_cargo("cargo llvm-cov report", &report, &envs)
}

/// `cargo llvm-cov show-env`の環境変数をcontinuous modeに切り替えて返す。
///
/// E2EのアプリはtesttyからSIGHUPで終了させられ、終了時の書き出しが行われずprofrawが空になる。
/// continuous modeではカウンタがprofrawへ直接mmapされるため、シグナルで終了しても残る。
fn coverage_envs() -> Result<Vec<(String, String)>, String> {
    // 計測付きbuildで通常の`target/`の成果物を作り直さないよう、cargo-llvm-covと同じ別dirを使う。
    let target_dir = std::env::current_dir()
        .map_err(|err| format!("failed to get the current directory: {err}"))?
        .join(Path::new("target").join("llvm-cov-target"));
    let output = Command::new("cargo")
        .args(["llvm-cov", "show-env"])
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .map_err(|err| format!("failed to start cargo llvm-cov show-env: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo llvm-cov show-env failed with status: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let mut envs = parse_show_env(&String::from_utf8_lossy(&output.stdout))?;
    let profile_file = envs
        .iter_mut()
        .find(|(key, _)| key == "LLVM_PROFILE_FILE")
        .ok_or_else(|| "cargo llvm-cov show-env did not set LLVM_PROFILE_FILE".to_string())?;
    if !profile_file.1.contains("%p") {
        return Err(format!(
            "unexpected LLVM_PROFILE_FILE from cargo llvm-cov show-env: {}",
            profile_file.1
        ));
    }
    profile_file.1 = profile_file.1.replacen("%p", "%c%p", 1);

    // Linuxのcontinuous modeはカウンタの再配置を有効にしたbuildでしか動かない。
    // show-envが`RUSTFLAGS`で計測を有効にする場合も、その値を残して追記する。
    let mut rustflags = match envs.iter().position(|(key, _)| key == "RUSTFLAGS") {
        Some(index) => envs.remove(index).1,
        None => std::env::var("RUSTFLAGS").unwrap_or_default(),
    };
    if !rustflags.is_empty() {
        rustflags.push(' ');
    }
    rustflags.push_str("-Cllvm-args=-runtime-counter-relocation");
    envs.push(("RUSTFLAGS".to_string(), rustflags));
    envs.push((
        "CARGO_TARGET_DIR".to_string(),
        target_dir.display().to_string(),
    ));
    Ok(envs)
}

/// `KEY=VALUE`の各行を読み、shellのsingle quoteで囲まれた値は外して返す。
fn parse_show_env(stdout: &str) -> Result<Vec<(String, String)>, String> {
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("unexpected cargo llvm-cov show-env line: {line}"))?;
            let value = match value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
            {
                Some(quoted) => quoted.replace("'\\''", "'"),
                None => value.to_string(),
            };
            Ok((key.to_string(), value))
        })
        .collect()
}

fn run_cargo(name: &str, args: &[&str], envs: &[(String, String)]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .envs(envs.iter().map(|(key, value)| (key, value)))
        .status()
        .map_err(|err| format!("failed to start {name}: {err}"))?;
    if !status.success() {
        return Err(format!("{name} failed with status: {status}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_env_values_are_unquoted() {
        let stdout = "LLVM_PROFILE_FILE='/repo/target/redmine-tui-%p-%24m.profraw'\n\
                      __CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS='-C\u{1f}instrument-coverage'\n\
                      CARGO_LLVM_COV=1\n\
                      QUOTED='it'\\''s'\n";

        assert_eq!(
            parse_show_env(stdout).unwrap(),
            vec![
                (
                    "LLVM_PROFILE_FILE".to_string(),
                    "/repo/target/redmine-tui-%p-%24m.profraw".to_string()
                ),
                (
                    "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS".to_string(),
                    "-C\u{1f}instrument-coverage".to_string()
                ),
                ("CARGO_LLVM_COV".to_string(), "1".to_string()),
                ("QUOTED".to_string(), "it's".to_string()),
            ]
        );
    }
}
