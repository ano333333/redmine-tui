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
    sync::{
        Arc,
        mpsc::{self, Sender},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};

use self::{
    app::{Action, Dispatcher, IssueState},
    clients::redmine::{DefaultRedmineClient, RedmineClient},
    components::{
        AppComponent,
        app::{AppEffect, EditorRequest, EditorResponse},
    },
    usecases::redmine::{
        apply_issue_property_diffs, fetch_issue_with_conflicts, load_initial_entities, upload_issue,
    },
    vos::{IssueId, IssuePropertyDiff},
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
    let client = Arc::new(DefaultRedmineClient::new(
        config.host_url,
        config.access_token,
    ));
    let actions = match runtime.block_on(load_initial_entities(client.as_ref())) {
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
                        client.clone(),
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
    client: Arc<DefaultRedmineClient>,
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
            client,
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
    client: Arc<DefaultRedmineClient>,
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
            let (_, state) = d
                .store()
                .get_issue(id)
                .expect("tried to upload unknown issue");
            if state != IssueState::Edited {
                panic!("uploading issue is not edited");
            }
            let diffs = d.store().get_issue_property_diffs(id).to_vec();
            runtime.spawn(async move {
                let action = issue_upload_action(client.as_ref(), id, &diffs).await;
                sender
                    .send(action)
                    .expect("Failed to send Action with mpsc::channel");
            });
        }
    }
    Ok(())
}

async fn issue_upload_action(
    client: &impl RedmineClient,
    id: IssueId,
    diffs: &[IssuePropertyDiff],
) -> Action {
    let (mut server_issue, conflicts) = match fetch_issue_with_conflicts(client, id, diffs).await {
        Ok(result) => result,
        Err(_) => return Action::FailIssueUpload { id },
    };
    if !conflicts.is_empty() {
        panic!("Issue property conflict resolution is not implemented");
    }

    apply_issue_property_diffs(&mut server_issue, diffs);
    if upload_issue(client, &server_issue).await.is_err() {
        return Action::FailIssueUpload { id };
    }

    Action::SyncIssue {
        issue: server_issue,
    }
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
    use std::sync::Mutex;

    use crate::clients::redmine::{RedmineClient, RedmineClientError, RedmineHttpError};
    use crate::entities::{
        Category, Issue, IssueStatus, Priority, Project, TargetVersion, TimeEntityActivity,
        Tracker, User,
    };
    use crate::test_support::sample_issue;
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crate::vos::{IssueId, IssuePropertyDiff, IssueStatusId};

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

    #[tokio::test]
    async fn issue_upload_uses_server_issue_as_merge_base() {
        let mut server_issue = sample_issue(
            1,
            "server subject",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        );
        server_issue.updated_on = crate::test_support::local_datetime("2026-08-23T12:00:00+09:00");
        server_issue.description = "original description".to_string();
        let client = IssueUploadClient::new(server_issue.clone());
        let diffs = vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "original description".to_string(),
            after: "local description".to_string(),
        })];

        let action = issue_upload_action(&client, 1.into(), &diffs).await;

        let Action::SyncIssue { issue } = action else {
            panic!("expected SyncIssue");
        };
        assert_eq!(issue.subject, "server subject");
        assert_eq!(issue.updated_on, server_issue.updated_on);
        assert_eq!(issue.description, "local description");
        let uploaded = client.uploaded.lock().unwrap();
        assert_eq!(uploaded.len(), 1);
        assert_eq!(uploaded[0].subject, issue.subject);
        assert_eq!(uploaded[0].description, issue.description);
        assert_eq!(uploaded[0].updated_on, issue.updated_on);
    }

    #[tokio::test]
    async fn issue_upload_returns_fail_action_when_fetch_fails() {
        let client = IssueUploadClient::failing_get();

        let action = issue_upload_action(&client, 1.into(), &[]).await;

        assert!(matches!(action, Action::FailIssueUpload { id } if id == IssueId::new(1)));
        assert!(client.uploaded.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn issue_upload_returns_fail_action_when_update_fails() {
        let server_issue = sample_issue(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        let client = IssueUploadClient::failing_update(server_issue);

        let action = issue_upload_action(&client, 1.into(), &[]).await;

        assert!(matches!(action, Action::FailIssueUpload { id } if id == IssueId::new(1)));
    }

    #[tokio::test]
    #[should_panic(expected = "Issue property conflict resolution is not implemented")]
    async fn issue_upload_panics_when_property_conflicts() {
        let mut server_issue =
            sample_issue(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        server_issue.description = "server description".to_string();
        let client = IssueUploadClient::new(server_issue);
        let diffs = vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "original description".to_string(),
            after: "local description".to_string(),
        })];

        issue_upload_action(&client, 1.into(), &diffs).await;
    }

    struct IssueUploadClient {
        issue: Option<Issue>,
        get_error: bool,
        update_error: bool,
        uploaded: Mutex<Vec<Issue>>,
    }

    impl IssueUploadClient {
        fn new(issue: Issue) -> Self {
            Self {
                issue: Some(issue),
                get_error: false,
                update_error: false,
                uploaded: Mutex::new(Vec::new()),
            }
        }

        fn failing_get() -> Self {
            Self {
                issue: None,
                get_error: true,
                update_error: false,
                uploaded: Mutex::new(Vec::new()),
            }
        }

        fn failing_update(issue: Issue) -> Self {
            Self {
                issue: Some(issue),
                get_error: false,
                update_error: true,
                uploaded: Mutex::new(Vec::new()),
            }
        }

        fn network_error() -> RedmineClientError {
            RedmineClientError::Network {
                reason: "offline".to_string(),
            }
        }
    }

    impl RedmineClient for IssueUploadClient {
        async fn get_issue(&self, _: IssueId) -> std::result::Result<Issue, RedmineClientError> {
            if self.get_error {
                return Err(Self::network_error());
            }
            Ok(self.issue.clone().expect("test issue must exist"))
        }

        async fn update_issue(&self, issue: &Issue) -> std::result::Result<(), RedmineClientError> {
            if self.update_error {
                return Err(Self::network_error());
            }
            self.uploaded.lock().unwrap().push(issue.clone());
            Ok(())
        }

        async fn get_categories(&self) -> std::result::Result<Vec<Category>, RedmineClientError> {
            unreachable!()
        }

        async fn get_issue_statuses(
            &self,
        ) -> std::result::Result<Vec<IssueStatus>, RedmineClientError> {
            unreachable!()
        }

        async fn get_priorities(&self) -> std::result::Result<Vec<Priority>, RedmineClientError> {
            unreachable!()
        }

        async fn get_projects(&self) -> std::result::Result<Vec<Project>, RedmineClientError> {
            unreachable!()
        }

        async fn get_target_versions(
            &self,
        ) -> std::result::Result<Vec<TargetVersion>, RedmineClientError> {
            unreachable!()
        }

        async fn get_time_entity_activities(
            &self,
        ) -> std::result::Result<Vec<TimeEntityActivity>, RedmineClientError> {
            unreachable!()
        }

        async fn get_trackers(&self) -> std::result::Result<Vec<Tracker>, RedmineClientError> {
            unreachable!()
        }

        async fn get_users(&self) -> std::result::Result<Vec<User>, RedmineClientError> {
            unreachable!()
        }
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

        async fn update_issue(&self, _: &Issue) -> std::result::Result<(), RedmineClientError> {
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
