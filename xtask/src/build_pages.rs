//! GitHub Pages 用の Trunk release build を実行し、成果物への認証情報混入と公開パスを検査する。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::seed_redmine::REDMINE_TUI_TEST_API_KEY;

pub(crate) fn build_pages(args: Vec<String>) -> Result<(), String> {
    let mut public_url = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--public-url" => {
                public_url = Some(
                    args.next()
                        .ok_or_else(|| "--public-url requires a URL".to_string())?,
                );
            }
            "--help" | "-h" => {
                println!("{}", crate::usage());
                return Ok(());
            }
            other => {
                return Err(format!(
                    "unknown build-pages option: {other}\n\n{}",
                    crate::usage()
                ));
            }
        }
    }
    let public_url =
        public_url.ok_or_else(|| format!("--public-url is required\n\n{}", crate::usage()))?;
    if public_url.is_empty() {
        return Err("--public-url cannot be empty".to_string());
    }

    let mut command = Command::new("trunk");
    command
        .arg("build")
        .arg("--release")
        .arg("--public-url")
        .arg(&public_url)
        // 色制御の環境変数によって Trunk の起動が失敗する場合があるため、引き継がない。
        .env_remove("NO_COLOR")
        .env_remove("TRUNK_NO_COLOR");
    let status = command.status().map_err(|err| {
        format!("failed to start trunk: {err}; run this command inside nix develop")
    })?;
    if !status.success() {
        return Err(format!("trunk release build failed with status: {status}"));
    }

    verify_pages_dist(Path::new("dist"), &public_url)
}

// 既知の Redmine 認証情報・環境変数の URL の混入と、公開パス外へのルート相対参照を検査する。
fn verify_pages_dist(dist: &Path, public_url: &str) -> Result<(), String> {
    let mut patterns = vec![(
        "default REDMINE_API_KEY",
        REDMINE_TUI_TEST_API_KEY.to_string(),
    )];
    for name in ["REDMINE_API_KEY", "REDMINE_URL"] {
        if let Ok(value) = std::env::var(name)
            && !value.is_empty()
        {
            patterns.push((name, value));
        }
    }
    patterns.push(("X-Redmine-API-Key", "X-Redmine-API-Key".to_string()));

    let mut files = Vec::new();
    collect_files(dist, &mut files)?;
    let mut violations = Vec::new();
    for file in files {
        let contents =
            fs::read(&file).map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        for (label, pattern) in &patterns {
            if contains_bytes(&contents, pattern.as_bytes()) {
                violations.push(format!("{}: {label}", file.display()));
            }
        }
    }

    let index_path = dist.join("index.html");
    let index = fs::read_to_string(&index_path)
        .map_err(|err| format!("failed to read {}: {err}", index_path.display()))?;
    for reference in index_root_references(&index) {
        if !reference.starts_with(public_url) {
            violations.push(format!(
                "{}: asset reference {reference:?} does not start with {public_url:?}",
                index_path.display()
            ));
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Pages artifact verification failed:\n{}",
            violations.join("\n")
        ))
    }
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|err| format!("failed to read {}: {err}", directory.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|err| format!("failed to read entry in {}: {err}", directory.display()))?
            .path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

// Trunk が出力する src/href/import のルート相対参照は、引用符の直後が `/` になる前提。
fn index_root_references(index: &str) -> Vec<String> {
    let chars: Vec<char> = index.chars().collect();
    let mut references = Vec::new();
    let mut start = 0;
    while start + 1 < chars.len() {
        let quote = chars[start];
        if (quote == '\'' || quote == '"') && chars[start + 1] == '/' {
            if let Some(end) = chars[start + 2..].iter().position(|ch| *ch == quote) {
                references.push(chars[start + 1..start + 2 + end].iter().collect());
                start += end + 2;
            }
        }
        start += 1;
    }
    references
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_root_references_extracts_quoted_paths() {
        let index = r#"<link href="/demo/app.wasm"><script src='/demo/app.js'></script><script>import init from "/demo/module.js";</script>"#;

        assert_eq!(
            index_root_references(index),
            vec!["/demo/app.wasm", "/demo/app.js", "/demo/module.js"]
        );
    }

    #[test]
    fn pages_verification_reports_secret_and_root_relative_asset_references() {
        let dist = std::env::temp_dir().join(format!("xtask-pages-{}", std::process::id()));
        fs::create_dir_all(&dist).unwrap();
        fs::write(
            dist.join("app.js"),
            "0123456789abcdef0123456789abcdef01234567",
        )
        .unwrap();
        fs::write(dist.join("index.html"), "<script src=\"/app.js\"></script>").unwrap();

        let error = verify_pages_dist(&dist, "/redmine-tui/").unwrap_err();
        assert!(error.contains("app.js: default REDMINE_API_KEY"));
        assert!(
            error.contains("asset reference \"/app.js\" does not start with \"/redmine-tui/\"")
        );
        fs::remove_dir_all(dist).unwrap();
    }
}
