mod clients;
mod components;
mod entities;
mod libs;
mod logging;
mod platform;
mod runner;
mod stores;
#[cfg(test)]
mod test_support;
mod usecases;
mod vos;
mod widgets;

use std::{cell::RefCell, process::ExitCode, rc::Rc, sync::Arc};

use self::{
    clients::redmine::DefaultRedmineClient,
    platform::editor::native::NativeTextEditor,
    platform::host::native::{NativePlatformHost, install_panic_hook},
    platform::redmine_config::redmine_connection_config_from_env,
    platform::runtime::tokio_spawner::TokioBackgroundSpawner,
    runner::{RunError, lifecycle::consume_initial_actions, run},
    stores::Dispatcher,
    usecases::redmine::load_initial_entities,
};

struct TerminalRestoreGuard;

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn main() -> ExitCode {
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
    if let Err(error) = logging::initialize_logging() {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    trace_dbg!("start");
    install_panic_hook();
    let mut host = NativePlatformHost::new(ratatui::init());
    // runのFutureが保持するeditor sessionを先に破棄し、以降のどのreturnでもterminalを復帰する。
    let _terminal_restore = TerminalRestoreGuard;
    let editor = NativeTextEditor::from_environment();
    match spawner.block_on(run(&mut host, &editor, &spawner, client, dispatcher)) {
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
