//! 共通runnerへ接続するまで、Web terminalの初期化と準備画面の描画だけを担う。

use ratatui::{
    Terminal,
    widgets::{Paragraph, Widget},
};
use ratzilla::{
    DomBackend,
    web_sys::{console, wasm_bindgen::JsValue},
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
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            report(format!("failed to initialize web terminal: {error}"));
            return;
        }
    };
    if let Err(error) = terminal.draw(|frame| {
        Paragraph::new("redmine-tui\nWeb demo is preparing")
            .render(frame.area(), frame.buffer_mut());
    }) {
        report(format!("failed to draw web demo: {error}"));
    }
}
