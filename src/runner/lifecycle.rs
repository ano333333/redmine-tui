//! runner loopから呼ぶapplication lifecycleの共通処理。

use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Duration};

use ratatui::{Frame, layout::Rect};

use crate::{
    components::AppComponent,
    platform::host::HostEvent,
    platform::runtime::{BackgroundCompletion, BackgroundSpawner},
    stores::{Action, Dispatcher},
    usecases::{UsecaseOutput, UsecaseRequest},
};

/// `now`が`last`より前、または`chrono::Duration`の範囲外ならゼロを返す。
pub(crate) fn tick_since(last: Duration, now: Duration) -> chrono::Duration {
    chrono::Duration::from_std(now.saturating_sub(last))
        .unwrap_or_else(|_| chrono::Duration::zero())
}

pub(crate) fn handle_host_event(
    event: HostEvent,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    area: Rect,
) -> bool {
    let should_continue = match event {
        HostEvent::Input(event) => app_component.handle_key_event(event, dispatcher.clone()),
        HostEvent::Ignored => true,
    };
    update(dispatcher.clone(), app_component, area);
    app_component.update(dispatcher.clone(), dispatcher.borrow().store(), area);
    should_continue
}

/// 受理したcompletionのActionをdispatchし、後続要求を`requests`の末尾に積む。
pub(crate) fn move_worker_action<S: BackgroundSpawner<Output = UsecaseOutput>>(
    spawner: &S,
    dispatcher: Rc<RefCell<Dispatcher>>,
    requests: &mut VecDeque<UsecaseRequest>,
) -> Option<String> {
    let mut worker_panic_message = None;
    while let Some(completion) = spawner.try_recv_completion() {
        match completion {
            BackgroundCompletion::Succeeded(output) => {
                // completionの受理順とtaskが生成したActionの順序を保ってmain thread上でdispatchする。
                for action in output.actions {
                    dispatcher.borrow_mut().dispatch(action);
                }
                requests.extend(output.requests);
            }
            BackgroundCompletion::Panicked { message } => {
                // Storeへ通常のActionとして流さず、runnerへ返してプロセスの異常終了を判断させる。
                worker_panic_message = Some(message);
            }
        }
    }
    worker_panic_message
}

pub(crate) fn consume_editor_worker_actions<S: BackgroundSpawner<Output = UsecaseOutput>>(
    spawner: &S,
    dispatcher: Rc<RefCell<Dispatcher>>,
    requests: &mut VecDeque<UsecaseRequest>,
) -> Option<String> {
    let worker_panic_message = move_worker_action(spawner, dispatcher.clone(), requests);
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
