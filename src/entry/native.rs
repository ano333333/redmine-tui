//! native版の依存を組み立て、共通runnerを起動する。

use std::{cell::RefCell, process::ExitCode, rc::Rc, sync::Arc};

use crate::{
    clients::{
        local_file::native::{NativeLocalFileClient, NativeTempDir},
        redmine::DefaultRedmineClient,
    },
    platform::app_config::app_config_from_env,
    platform::editor::native::NativeTextEditor,
    platform::host::native::{NativePlatformHost, install_panic_hook},
    platform::redmine_config::redmine_connection_config_from_env,
    platform::runtime::tokio_spawner::TokioBackgroundSpawner,
    platform::terminal_image::image_display_from_config,
    runner::{RunError, lifecycle::consume_initial_actions, run as run_app},
    stores::{Dispatcher, SettingsAction},
    trace_dbg,
    usecases::{UsecaseClients, redmine::load_initial_entities},
};

struct TerminalRestoreGuard;

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

pub(crate) fn run() -> ExitCode {
    let spawner = match TokioBackgroundSpawner::new() {
        Ok(spawner) => spawner,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
    let config = match redmine_connection_config_from_env() {
        Ok(config) => config,
        Err(reason) => {
            eprintln!("failed to read Redmine connection environment: {reason}");
            return ExitCode::FAILURE;
        }
    };
    let app_config = match app_config_from_env() {
        Ok(config) => config,
        Err(reason) => {
            eprintln!("failed to read config file: {reason}");
            return ExitCode::FAILURE;
        }
    };
    let temp_dir = match NativeTempDir::create() {
        Ok(temp_dir) => temp_dir,
        Err(error) => {
            eprintln!("failed to create a temporary directory: {error}");
            return ExitCode::FAILURE;
        }
    };
    let client = Arc::new(DefaultRedmineClient::new(
        config.host_url,
        config.access_token,
    ));
    let actions = match spawner.block_on(load_initial_entities(client.as_ref())) {
        Ok(actions) => actions,
        Err(error) => {
            eprintln!("failed to load initial entities from Redmine: {error}");
            return ExitCode::FAILURE;
        }
    };
    consume_initial_actions(dispatcher.clone(), actions);
    if let Err(error) = crate::logging::initialize_logging() {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    trace_dbg!("start");
    install_panic_hook();
    let mut host = NativePlatformHost::new(ratatui::init());
    // runのFutureが保持するeditor sessionを先に破棄し、以降のどのreturnでもterminalを復帰する。
    let _terminal_restore = TerminalRestoreGuard;
    let image_display = image_display_from_config(app_config.image_enabled);
    consume_initial_actions(
        dispatcher.clone(),
        vec![SettingsAction::SetImageDisplay(image_display).into()],
    );
    let editor = NativeTextEditor::from_environment();
    let clients = UsecaseClients {
        redmine: client,
        local_file: Arc::new(NativeLocalFileClient::new(&temp_dir)),
    };
    match spawner.block_on(run_app(&mut host, &editor, &spawner, clients, dispatcher)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(RunError::WorkerPanicked(message)) => {
            eprintln!("worker task panicked: {message}");
            ExitCode::FAILURE
        }
        Err(RunError::Draw(error) | RunError::Input(error)) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
