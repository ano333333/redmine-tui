mod app;
mod components;
mod entities;
mod libs;
mod logging;
#[cfg(test)]
mod test_support;
mod vos;
mod widgets;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{DefaultTerminal, Frame, layout::Rect};
use std::{
    cell::RefCell,
    env, fs,
    io::Result,
    process::Command,
    rc::Rc,
    sync::mpsc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};

use self::{
    app::{Action, Dispatcher},
    components::{
        AppComponent,
        app::{AppEffect, EditorRequest, EditorResponse},
    },
};

fn main() -> Result<()> {
    logging::initialize_logging()?;
    trace_dbg!("start");
    let runtime = init_tokio_runtime()?;
    let (worker_action_tx, worker_action_rx) = mpsc::channel::<Action>();
    let mut terminal = ratatui::init();
    let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
    init_store(dispatcher.clone());
    let mut app_component = AppComponent::new(dispatcher.clone());
    app_component.update(
        dispatcher.clone(),
        dispatcher.borrow().store(),
        terminal.get_frame().area(),
    );
    loop {
        move_worker_action(&worker_action_tx, dispatcher.clone());
        update(
            dispatcher.clone(),
            &mut app_component,
            terminal.get_frame().area(),
        );
        if let Some(e) = terminal
            .draw(|f| draw(f, &app_component, dispatcher.clone()))
            .err()
        {
            trace_dbg!(level: tracing::Level::ERROR, "failed to draw frame");
            return Err(e);
        }
        match event::read() {
            Ok(event) => {
                if !handle_key_event(event, &mut terminal, &mut app_component, dispatcher.clone()) {
                    break;
                }
            }
            Err(e) => {
                trace_dbg!(level: tracing::Level::ERROR, "failed to read event");
                return Err(e);
            }
        }
    }
    ratatui::restore();
    trace_dbg!("done");
    Ok(())
}

fn init_tokio_runtime() -> Result<Runtime> {
    TokioRuntimeBuilder::new_multi_thread().enable_all().build()
}

fn move_worker_action(tx: &mpsc::Receiver<Action>, dispatcher: Rc<RefCell<Dispatcher>>) {
    while let Ok(action) = tx.try_recv() {
        dispatcher.borrow_mut().dispatch(action);
    }
}

fn update(dispatcher: Rc<RefCell<Dispatcher>>, app_component: &mut AppComponent, area: Rect) {
    while dispatcher.borrow().consume_actinos_len() > 0 {
        dispatcher.borrow_mut().consume_action();
        app_component.update(dispatcher.clone(), dispatcher.borrow().store(), area);
    }
}

fn handle_key_event(
    event: Event,
    terminal: &mut DefaultTerminal,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
) -> bool {
    if let Event::Key(key) = event {
        if key.code == KeyCode::Char('q') {
            return false;
        }
        app_component.process_event(event, dispatcher.clone());
    }
    let size = terminal.size().expect("failed to get terminal size");
    let rect = Rect::new(0, 0, size.width, size.height);
    app_component.update(dispatcher.clone(), dispatcher.borrow().store(), rect);
    if let Some(effect) = app_component.take_effect()
        && let Err(err) = handle_app_effect(effect, terminal, app_component, dispatcher.clone())
    {
        tracing::event!(
            target: module_path!(),
            tracing::Level::ERROR,
            error = %err,
            "failed to handle app effect"
        );
    }
    true
}

fn handle_app_effect(
    effect: AppEffect,
    terminal: &mut DefaultTerminal,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
) -> Result<()> {
    match effect {
        AppEffect::OpenEditor(request) => {
            let response = run_editor(terminal, request)?;
            app_component.handle_editor_response(response);
            let size = terminal.size().expect("failed to get terminal size");
            let rect = Rect::new(0, 0, size.width, size.height);
            app_component.update(dispatcher.clone(), dispatcher.borrow().store(), rect);
        }
    }
    Ok(())
}

fn run_editor(terminal: &mut DefaultTerminal, request: EditorRequest) -> Result<EditorResponse> {
    let filename = format!(
        "redmine-tui-editor-{}.md",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let path = env::temp_dir().join(filename);
    fs::write(&path, &request.initial_text)?;

    // FIXME: nvim以外に対応
    let editor = env::var("VISUAL")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "nvim".to_string());

    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    let status = Command::new(&editor).arg(&path).status();
    let reenter_result = execute!(std::io::stdout(), EnterAlternateScreen);
    let raw_mode_result = enable_raw_mode();
    terminal.clear()?;

    reenter_result?;
    raw_mode_result?;
    status?;

    let edited = fs::read_to_string(&path)?;
    let _ = fs::remove_file(&path);
    Ok(EditorResponse {
        edited_text: edited,
    })
}

fn draw(frame: &mut Frame, app_component: &AppComponent, dispatcher: Rc<RefCell<Dispatcher>>) {
    app_component.render(dispatcher.borrow().store(), frame, frame.area());
}

fn init_store(dispatcher: Rc<RefCell<Dispatcher>>) {
    let mut d = dispatcher.borrow_mut();
    d.dispatch(Action::LoadUsers);
    d.dispatch(Action::LoadIssueStatuses);
    d.dispatch(Action::LoadPriorities);
    d.dispatch(Action::LoadProjects);
    d.dispatch(Action::LoadTrackers);
    d.dispatch(Action::LoadTargetVersions);
    d.dispatch(Action::LoadCategories);
    d.dispatch(Action::LoadTimeEntityActivities);
    d.dispatch(Action::LoadIssue { id: 1.into() });
    d.dispatch(Action::LoadIssue { id: 2.into() });
    d.dispatch(Action::LoadIssue { id: 3.into() });
    d.dispatch(Action::LoadJournal { id: 1.into() });
    d.dispatch(Action::LoadJournal { id: 2.into() });
    d.dispatch(Action::LoadJournal { id: 3.into() });
    while d.consume_actinos_len() > 0 {
        d.consume_action();
    }
}
