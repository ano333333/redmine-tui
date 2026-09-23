//! runner loopから呼ぶapplication lifecycleの共通処理。

use std::{cell::RefCell, rc::Rc};

use ratatui::{Frame, layout::Rect};

use crate::{
    components::AppComponent,
    platform::runtime::{BackgroundCompletion, BackgroundSpawner},
    stores::{Action, Dispatcher},
};

/// `now`が`last`より前でないことを前提とし、`chrono::Duration`の範囲外ならゼロを返す。
pub(crate) fn tick_since(last: std::time::Instant, now: std::time::Instant) -> chrono::Duration {
    chrono::Duration::from_std(now.duration_since(last))
        .unwrap_or_else(|_| chrono::Duration::zero())
}

pub(crate) fn move_worker_action<S: BackgroundSpawner>(
    spawner: &S,
    dispatcher: Rc<RefCell<Dispatcher>>,
) -> Option<String> {
    let mut worker_panic_message = None;
    while let Some(completion) = spawner.try_recv_completion() {
        match completion {
            BackgroundCompletion::Succeeded(actions) => {
                // completionの受理順とtaskが生成したActionの順序を保ってmain thread上でdispatchする。
                for action in actions {
                    dispatcher.borrow_mut().dispatch(action);
                }
            }
            BackgroundCompletion::Panicked { message } => {
                // Storeへ通常のActionとして流さず、runnerへ返してプロセスの異常終了を判断させる。
                worker_panic_message = Some(message);
            }
        }
    }
    worker_panic_message
}

pub(crate) fn consume_editor_worker_actions<S: BackgroundSpawner>(
    spawner: &S,
    dispatcher: Rc<RefCell<Dispatcher>>,
) -> Option<String> {
    let worker_panic_message = move_worker_action(spawner, dispatcher.clone());
    while dispatcher.borrow().consume_actinos_len() > 0 {
        dispatcher.borrow_mut().consume_action();
    }
    worker_panic_message
}

pub(crate) fn update(
    dispatcher: Rc<RefCell<Dispatcher>>,
    app_component: &mut AppComponent,
    area: Rect,
) {
    while dispatcher.borrow().consume_actinos_len() > 0 {
        dispatcher.borrow_mut().consume_action();
        app_component.update(dispatcher.clone(), dispatcher.borrow().store(), area);
    }
}

pub(crate) fn draw(
    frame: &mut Frame,
    app_component: &AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
) {
    app_component.render(dispatcher.borrow().store(), frame, frame.area());
}

pub(crate) fn consume_initial_actions(dispatcher: Rc<RefCell<Dispatcher>>, actions: Vec<Action>) {
    let mut d = dispatcher.borrow_mut();
    for action in actions {
        d.dispatch(action);
    }
    while d.consume_actinos_len() > 0 {
        d.consume_action();
    }
}
