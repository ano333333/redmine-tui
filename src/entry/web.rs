//! Web版の依存を組み立て、初期データの読み込み後に共通runnerを起動する。

use std::{cell::RefCell, rc::Rc, sync::Arc};

use ratatui::Terminal;
use ratzilla::{
    DomBackend,
    web_sys::{console, wasm_bindgen::JsValue},
};
use wasm_bindgen_futures::spawn_local;

use crate::{
    clients::redmine::demo::DemoRedmineClient,
    platform::{
        editor::web::WebTextEditor,
        host::web::{WebBackend, WebPlatformHost},
        runtime::web_spawner::WebBackgroundSpawner,
    },
    runner::{RunError, lifecycle::consume_initial_actions, run as run_app},
    stores::Dispatcher,
    usecases::redmine::load_initial_entities,
};

fn report(message: impl AsRef<str>) {
    console::error_1(&JsValue::from_str(message.as_ref()));
}

pub(crate) fn run() {
    std::panic::set_hook(Box::new(|info| {
        report(format!("web demo panic: {info}"));
    }));

    let backend = match DomBackend::new() {
        Ok(backend) => backend,
        Err(error) => {
            report(format!(
                "failed to initialize web terminal backend: {error}"
            ));
            return;
        }
    };
    let terminal = match Terminal::new(WebBackend::new(backend)) {
        Ok(terminal) => terminal,
        Err(error) => {
            report(format!("failed to initialize web terminal: {error}"));
            return;
        }
    };
    let mut host = WebPlatformHost::new(terminal);
    let client = Arc::new(DemoRedmineClient::new());
    let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
    let spawner = WebBackgroundSpawner::new();
    let editor = WebTextEditor;
    spawn_local(async move {
        let actions = match load_initial_entities(client.as_ref()).await {
            Ok(actions) => actions,
            Err(error) => {
                report(format!("failed to load initial demo entities: {error}"));
                return;
            }
        };
        consume_initial_actions(dispatcher.clone(), actions);
        if let Err(error) = run_app(&mut host, &editor, &spawner, client, dispatcher).await {
            match error {
                RunError::WorkerPanicked(message) => {
                    report(format!("worker task panicked: {message}"))
                }
                RunError::Draw(error) | RunError::Input(error) => report(error.to_string()),
            }
        }
    });
}
