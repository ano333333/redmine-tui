mod app;
mod clients;
mod components;
mod entities;
mod libs;
mod logging;
#[cfg(test)]
mod test_support;
mod usecases;
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
    process::{Command, ExitCode},
    rc::Rc,
    sync::mpsc::{self, Sender},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};

use self::{
    app::{Action, Dispatcher, IssueState},
    clients::redmine::DefaultRedmineClient,
    components::{
        AppComponent,
        app::{AppEffect, EditorRequest, EditorResponse},
    },
    usecases::redmine::load_initial_entities,
};

const TICK_RATE_MS: u64 = 250;
const REDMINE_API_KEY_ENV: &str = "REDMINE_API_KEY";
const REDMINE_URL_ENV: &str = "REDMINE_URL";
const REDMINE_PORT_ENV: &str = "REDMINE_PORT";

fn main() -> ExitCode {
    let runtime = match init_tokio_runtime() {
        Ok(runtime) => runtime,
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
    let client = DefaultRedmineClient::new(config.host_url, config.access_token);
    let actions = match runtime.block_on(load_initial_entities(&client)) {
        Ok(actions) => actions,
        Err(error) => {
            eprintln!("failed to load initial entities from Redmine: {error}");
            return ExitCode::FAILURE;
        }
    };
    consume_initial_actions(dispatcher.clone(), actions);
    init_fixture_issues_and_journals(dispatcher.clone());
    if let Err(error) = logging::initialize_logging() {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    trace_dbg!("start");
    let (worker_action_tx, worker_action_rx) = mpsc::channel::<Action>();
    let mut terminal = ratatui::init();
    let mut app_component = AppComponent::new(dispatcher.clone(), None);
    app_component.update(
        dispatcher.clone(),
        dispatcher.borrow().store(),
        terminal.get_frame().area(),
    );
    let tick_rate = std::time::Duration::from_millis(TICK_RATE_MS);
    loop {
        move_worker_action(&worker_action_rx, dispatcher.clone());
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
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        if event::poll(tick_rate).unwrap() {
            match event::read() {
                Ok(event) => {
                    if !handle_key_event(
                        event,
                        &mut terminal,
                        &mut app_component,
                        dispatcher.clone(),
                        &runtime,
                        worker_action_tx.clone(),
                    ) {
                        break;
                    }
                }
                Err(e) => {
                    trace_dbg!(level: tracing::Level::ERROR, "failed to read event");
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    ratatui::restore();
    trace_dbg!("done");
    ExitCode::SUCCESS
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
    runtime: &Runtime,
    sender: Sender<Action>,
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
        && let Err(err) = handle_app_effect(
            effect,
            terminal,
            app_component,
            dispatcher.clone(),
            runtime,
            sender,
        )
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
    runtime: &Runtime,
    sender: mpsc::Sender<Action>,
) -> Result<()> {
    match effect {
        AppEffect::OpenEditor(request) => {
            let response = run_editor(terminal, request)?;
            app_component.handle_editor_response(response);
            let size = terminal.size().expect("failed to get terminal size");
            let rect = Rect::new(0, 0, size.width, size.height);
            app_component.update(dispatcher.clone(), dispatcher.borrow().store(), rect);
        }
        AppEffect::StartIssueUpload(id) => {
            let mut d = dispatcher.borrow_mut();
            d.dispatch(Action::StartIssueUpload { id });
            let (issue, state) = d
                .store()
                .get_issue(id)
                .expect("tried to upload unknown issue");
            if state != IssueState::Edited {
                panic!("uploading issue is not edited");
            }
            let issue = issue.clone();
            // TODO: issue_property_diffsの取得とマージ
            runtime.spawn(async move {
                // TODO: Redmineから指定issueの読み込み
                tokio::time::sleep(Duration::from_secs(3)).await;
                sender
                    .send(Action::SyncIssue { issue })
                    .expect("Failed to send Action with mpsc::channel");
            });
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RedmineConnectionConfig {
    host_url: String,
    access_token: String,
}

fn redmine_connection_config_from_env() -> std::result::Result<RedmineConnectionConfig, String> {
    read_redmine_connection_config(|name| match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid Unicode")),
    })
}

fn read_redmine_connection_config<F>(
    mut read_env: F,
) -> std::result::Result<RedmineConnectionConfig, String>
where
    F: FnMut(&'static str) -> std::result::Result<Option<String>, String>,
{
    let access_token = required_env_value(read_env(REDMINE_API_KEY_ENV)?, REDMINE_API_KEY_ENV)?;
    let host_url = read_env(REDMINE_URL_ENV)?
        .filter(|value| !value.is_empty())
        .map(Ok)
        .unwrap_or_else(|| default_redmine_url(&mut read_env))?;

    Ok(RedmineConnectionConfig {
        host_url,
        access_token,
    })
}

fn required_env_value(
    value: Option<String>,
    name: &'static str,
) -> std::result::Result<String, String> {
    match value {
        Some(value) if value.is_empty() => Err(format!("{name} is empty")),
        Some(value) => Ok(value),
        None => Err(format!("{name} is not set")),
    }
}

fn default_redmine_url<F>(read_env: &mut F) -> std::result::Result<String, String>
where
    F: FnMut(&'static str) -> std::result::Result<Option<String>, String>,
{
    let port = read_env(REDMINE_PORT_ENV)?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "8080".to_string());
    Ok(format!("http://127.0.0.1:{port}"))
}

fn consume_initial_actions(dispatcher: Rc<RefCell<Dispatcher>>, actions: Vec<Action>) {
    let mut d = dispatcher.borrow_mut();
    for action in actions {
        d.dispatch(action);
    }
    while d.consume_actinos_len() > 0 {
        d.consume_action();
    }
}

fn init_fixture_issues_and_journals(dispatcher: Rc<RefCell<Dispatcher>>) {
    let mut d = dispatcher.borrow_mut();
    dispatch_fixture_issues_and_journals(&mut d);
    while d.consume_actinos_len() > 0 {
        d.consume_action();
    }
}

fn dispatch_fixture_issues_and_journals(d: &mut Dispatcher) {
    d.dispatch(Action::LoadIssue { id: 1.into() });
    d.dispatch(Action::LoadIssue { id: 2.into() });
    d.dispatch(Action::LoadIssue { id: 3.into() });
    d.dispatch(Action::LoadJournal { id: 1.into() });
    d.dispatch(Action::LoadJournal { id: 2.into() });
    d.dispatch(Action::LoadJournal { id: 3.into() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::redmine::{RedmineClient, RedmineClientError, RedmineHttpError};
    use crate::entities::{
        Category, Issue, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity,
        Tracker, User,
    };
    use crate::vos::IssueId;

    #[test]
    fn redmine_connection_config_requires_api_key() {
        let error = expect_string_error(read_redmine_connection_config(|_| Ok(None)));

        assert_eq!(error, "REDMINE_API_KEY is not set");
    }

    #[test]
    fn redmine_connection_config_rejects_empty_api_key() {
        let error = expect_string_error(read_redmine_connection_config(|name| {
            Ok(match name {
                REDMINE_API_KEY_ENV => Some(String::new()),
                _ => None,
            })
        }));

        assert_eq!(error, "REDMINE_API_KEY is empty");
    }

    #[test]
    fn redmine_connection_config_uses_default_url_when_url_is_not_set() {
        let config = read_redmine_connection_config(|name| {
            Ok(match name {
                REDMINE_API_KEY_ENV => Some("secret-token".to_string()),
                _ => None,
            })
        })
        .unwrap();

        assert_eq!(config.access_token, "secret-token");
        assert_eq!(config.host_url, "http://127.0.0.1:8080");
    }

    #[test]
    fn load_initial_entities_returns_redmine_client_error() {
        let runtime = init_tokio_runtime().unwrap();
        let client = FailingClient;

        let error = match runtime.block_on(load_initial_entities(&client)) {
            Ok(_) => panic!("load initial entities succeeded"),
            Err(error) => error,
        };

        assert_eq!(
            error.to_string(),
            "unauthorized: GET http://redmine.invalid/users.json returned 401 with body: {\"error\":\"failed\"}"
        );
    }

    fn expect_string_error<T>(result: std::result::Result<T, String>) -> String {
        match result {
            Ok(_) => panic!("redmine connection config read succeeded"),
            Err(error) => error,
        }
    }

    struct FailingClient;

    impl FailingClient {
        fn unauthorized(&self) -> RedmineClientError {
            RedmineClientError::Unauthorized {
                context: RedmineHttpError {
                    method: "GET".to_string(),
                    url: "http://redmine.invalid/users.json".to_string(),
                    status_code: 401,
                    response_body: r#"{"error":"failed"}"#.to_string(),
                },
            }
        }
    }

    impl RedmineClient for FailingClient {
        async fn get_categories(&self) -> std::result::Result<Vec<Category>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_issue(&self, _: IssueId) -> std::result::Result<Issue, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_issue_statuses(
            &self,
        ) -> std::result::Result<Vec<IssueStatus>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_priorities(&self) -> std::result::Result<Vec<Priority>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_projects(&self) -> std::result::Result<Vec<Project>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_target_versions(
            &self,
        ) -> std::result::Result<Vec<TargetVersion>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_time_entity_activities(
            &self,
        ) -> std::result::Result<Vec<TimeEntityActivity>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_trackers(&self) -> std::result::Result<Vec<Tracker>, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn get_users(&self) -> std::result::Result<Vec<User>, RedmineClientError> {
            Err(self.unauthorized())
        }
    }
}
