#[cfg(feature = "native")]
use directories::ProjectDirs;
#[cfg(feature = "native")]
use lazy_static::lazy_static;
#[cfg(feature = "native")]
use std::{io::Result, path::PathBuf};
#[cfg(feature = "native")]
use tracing_error::ErrorLayer;
#[cfg(any(feature = "native", feature = "web-demo"))]
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "web-demo")]
pub fn initialize_logging() {
    let layer = tracing_subscriber::fmt::layer()
        // tracing の level に応じて browser の console.info/warn/error などへ振り分ける。
        .with_writer(tracing_web::MakeWebConsoleWriter::new())
        .with_ansi(false)
        // fmt の既定の時刻取得は std::time::SystemTime を使うが、
        // wasm32-unknown-unknown では利用できず panic するため無効にする。
        .without_time()
        .with_filter(tracing_subscriber::filter::EnvFilter::new(format!(
            "{}=debug",
            env!("CARGO_CRATE_NAME")
        )));
    tracing_subscriber::registry().with(layer).init();
}

#[cfg(feature = "native")]
lazy_static! {
    pub static ref PROJECT_NAME: String = env!("CARGO_CRATE_NAME").to_uppercase().to_string();
    pub static ref DATA_FOLDER: Option<PathBuf> =
        std::env::var(format!("{}_DATA", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref LOG_ENV: String = format!("{}_LOGLEVEL", PROJECT_NAME.clone());
    pub static ref LOG_FILE: String = format!("{}.log", env!("CARGO_PKG_NAME"));
}

#[cfg(feature = "native")]
fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "kdheepak", env!("CARGO_PKG_NAME"))
}

#[cfg(feature = "native")]
pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    };
    directory
}

#[cfg(feature = "native")]
pub fn initialize_logging() -> Result<()> {
    let directory = get_data_dir();
    std::fs::create_dir_all(directory.clone())?;
    let log_path = directory.join(LOG_FILE.clone());
    let log_file = std::fs::File::create(log_path)?;
    let log_filter = std::env::var("RUST_LOG")
        .or_else(|_| std::env::var(LOG_ENV.clone()))
        .unwrap_or_else(|_| format!("{}=info", env!("CARGO_CRATE_NAME")));
    let file_subscriber = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_filter(tracing_subscriber::filter::EnvFilter::builder().parse_lossy(log_filter));
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .init();
    Ok(())
}

#[macro_export]
macro_rules! trace_dbg {
    (target: $target:expr, level: $level:expr, $ex:expr) => {{
        match $ex {
            value => {
                tracing::event!(target: $target, $level, ?value, stringify!($ex));
                value
            }
        }
    }};
    (level: $level:expr, $ex:expr) => {
        trace_dbg!(target: module_path!(), level: $level, $ex)
    };
    (target: $target:expr, $ex:expr) => {
        trace_dbg!(target: $target, level: tracing::Level::DEBUG, $ex)
    };
    ($ex:expr) => {
        trace_dbg!(level: tracing::Level::DEBUG, $ex)
    };
}
