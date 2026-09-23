//! native版とWeb版でapplication lifecycleを共有するrunner。

use std::{cell::RefCell, io, rc::Rc, sync::Arc, time::Duration};

use crate::{
    clients::redmine::RedmineClient,
    components::AppComponent,
    platform::{editor::TextEditor, host::PlatformHost, runtime::BackgroundSpawner},
    stores::Dispatcher,
    trace_dbg,
};

pub(crate) mod effect;
pub(crate) mod lifecycle;

use effect::{EditorSession, handle_app_effect, handle_editor_failure};
use lifecycle::{
    consume_editor_worker_actions, draw, handle_host_event, move_worker_action, tick_since, update,
};

const TICK_RATE_MS: u64 = 250;

#[derive(Debug)]
pub(crate) enum RunError {
    WorkerPanicked(String),
    Draw(io::Error),
    Input(io::Error),
}

pub(crate) async fn run<H, E, S, C>(
    host: &mut H,
    editor: &E,
    spawner: &S,
    client: Arc<C>,
    dispatcher: Rc<RefCell<Dispatcher>>,
) -> Result<(), RunError>
where
    H: PlatformHost,
    E: TextEditor,
    S: BackgroundSpawner,
    C: RedmineClient + Send + Sync + 'static,
{
    let mut app_component = AppComponent::new(dispatcher.clone(), None);
    app_component.update(dispatcher.clone(), dispatcher.borrow().store(), host.area());
    let tick_rate = Duration::from_millis(TICK_RATE_MS);
    let mut last_tick = host.elapsed();
    let mut editor_session: Option<EditorSession<'_>> = None;
    loop {
        // editor中もworker完了はStoreへ取り込むが、Component更新・描画・入力と
        // Noticeの経過時間更新はeditor終了まで遅延する。
        if let Some(session) = editor_session.as_mut() {
            if let Some(message) = consume_editor_worker_actions(spawner, dispatcher.clone()) {
                return Err(RunError::WorkerPanicked(message));
            }
            let outcome = match session.poll_completion() {
                None => {
                    host.wait(tick_rate).await;
                    continue;
                }
                Some(outcome) => outcome,
            };
            editor_session = None;
            match host.resume_after_editor() {
                Ok(()) => match outcome {
                    Ok(outcome) => app_component.handle_editor_response(outcome),
                    Err(error) => {
                        handle_editor_failure(
                            &mut app_component,
                            dispatcher.clone(),
                            &error,
                            "editor failed",
                        );
                    }
                },
                Err(error) => {
                    handle_editor_failure(
                        &mut app_component,
                        dispatcher.clone(),
                        &error,
                        "failed to restore terminal after editor",
                    );
                }
            }
            // editor中にStoreへ適用したActionを、描画再開前にComponentへ反映する。
            app_component.update(dispatcher.clone(), dispatcher.borrow().store(), host.area());
            // editor滞在時間を次のNotice tickへ混ぜないよう、通常loopへ戻る前にresetする。
            last_tick = host.elapsed();
        }
        if let Some(message) = move_worker_action(spawner, dispatcher.clone()) {
            return Err(RunError::WorkerPanicked(message));
        }
        let now = host.elapsed();
        let tick = tick_since(last_tick, now);
        last_tick = now;
        dispatcher.borrow_mut().update_store(tick);
        update(dispatcher.clone(), &mut app_component, host.area());
        if let Some(effect) = app_component.take_effect() {
            handle_app_effect(
                effect,
                &mut app_component,
                dispatcher.clone(),
                spawner,
                client.clone(),
                editor,
                &mut editor_session,
                host,
            );
        }
        if editor_session.is_some() {
            continue;
        }
        if let Err(error) = host.draw(|frame| draw(frame, &app_component, dispatcher.clone())) {
            trace_dbg!(level: tracing::Level::ERROR, "failed to draw frame");
            return Err(RunError::Draw(error));
        }
        match host.next_event(tick_rate).await {
            Ok(None) => {}
            Ok(Some(event)) => {
                if !handle_host_event(event, &mut app_component, dispatcher.clone(), host.area()) {
                    break;
                }
            }
            Err(error) => {
                trace_dbg!(level: tracing::Level::ERROR, "failed to read event");
                return Err(RunError::Input(error));
            }
        }
    }
    trace_dbg!("done");
    Ok(())
}
