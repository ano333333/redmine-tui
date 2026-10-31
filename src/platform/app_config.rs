//! native版の起動時に、`$XDG_CONFIG_HOME/redmine-tui/config.toml`から利用者の設定を読み込む。

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";
const HOME_ENV: &str = "HOME";
const CONFIG_FILE_PATH: &str = "redmine-tui/config.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AppConfig {
    /// 利用者が画像表示を有効にしたか。terminalが対応しているかは含まない。
    pub(crate) image_enabled: bool,
}

#[derive(Deserialize)]
struct ConfigFile {
    image: Option<ImageSection>,
}

#[derive(Deserialize)]
struct ImageSection {
    #[serde(default)]
    enable: bool,
}

/// 設定ファイルがない場合は既定値を返す。ファイルはあるのに読めない場合はエラーにする。
pub(crate) fn app_config_from_env() -> Result<AppConfig, String> {
    let Some(path) = config_file_path(|name| env::var_os(name).map(PathBuf::from)) else {
        return Ok(AppConfig::default());
    };
    match fs::read_to_string(&path) {
        Ok(content) => parse_app_config(&content).map_err(|reason| describe(&path, reason)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(error) => Err(describe(&path, error.to_string())),
    }
}

fn config_file_path<F>(mut read_env: F) -> Option<PathBuf>
where
    F: FnMut(&'static str) -> Option<PathBuf>,
{
    let config_home = read_env(XDG_CONFIG_HOME_ENV)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            read_env(HOME_ENV)
                .filter(|path| !path.as_os_str().is_empty())
                .map(|home| home.join(".config"))
        })?;
    Some(config_home.join(CONFIG_FILE_PATH))
}

fn parse_app_config(content: &str) -> Result<AppConfig, String> {
    let file: ConfigFile = toml::from_str(content).map_err(|error| error.to_string())?;
    Ok(AppConfig {
        image_enabled: file.image.is_some_and(|image| image.enable),
    })
}

fn describe(path: &Path, reason: String) -> String {
    format!("{}: {reason}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_path_prefers_xdg_config_home() {
        let path = config_file_path(|name| match name {
            XDG_CONFIG_HOME_ENV => Some(PathBuf::from("/xdg")),
            HOME_ENV => Some(PathBuf::from("/home/user")),
            _ => None,
        });

        assert_eq!(path, Some(PathBuf::from("/xdg/redmine-tui/config.toml")));
    }

    #[test]
    fn config_file_path_falls_back_to_home_when_xdg_config_home_is_empty() {
        let path = config_file_path(|name| match name {
            XDG_CONFIG_HOME_ENV => Some(PathBuf::new()),
            HOME_ENV => Some(PathBuf::from("/home/user")),
            _ => None,
        });

        assert_eq!(
            path,
            Some(PathBuf::from("/home/user/.config/redmine-tui/config.toml"))
        );
    }

    #[test]
    fn config_file_path_is_none_without_xdg_config_home_and_home() {
        assert_eq!(config_file_path(|_| None), None);
    }

    #[test]
    fn image_section_enables_image_display() {
        let config = parse_app_config("[image]\nenable = true\n").unwrap();

        assert!(config.image_enabled);
    }

    #[test]
    fn missing_image_section_disables_image_display() {
        let config = parse_app_config("").unwrap();

        assert!(!config.image_enabled);
    }

    #[test]
    fn image_section_without_enable_disables_image_display() {
        let config = parse_app_config("[image]\n").unwrap();

        assert!(!config.image_enabled);
    }

    #[test]
    fn invalid_toml_is_an_error() {
        assert!(parse_app_config("[image\n").is_err());
    }

    #[test]
    fn enable_with_wrong_type_is_an_error() {
        assert!(parse_app_config("[image]\nenable = \"yes\"\n").is_err());
    }
}
