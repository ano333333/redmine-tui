mod clients;
mod components;
mod entities;
mod libs;
mod logging;
mod stores;
#[cfg(test)]
mod test_support;
mod usecases;
mod vos;
mod widgets;

use crossterm::{
    event::{self, Event},
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
    sync::{Arc, mpsc},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};

use self::{
    clients::redmine::{DefaultRedmineClient, RedmineClient},
    components::{
        AppComponent,
        app::{AppEffect, EditorRequest, EditorResponse},
    },
    stores::{Action, Dispatcher, IssueAction, IssueState},
    usecases::redmine::{
        apply_issue_property_diffs, fetch_issue, fetch_issue_with_conflicts,
        fetch_project_issues_page, load_initial_entities, upload_issue,
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
        let size = terminal.size().expect("failed to get terminal size");
        update(
            dispatcher.clone(),
            &mut app_component,
            area_from_terminal_size(size.width, size.height),
        );
        let effect = app_component.take_effect();
        if let Some(effect) = effect
            && let Err(err) = handle_app_effect(
                effect,
                &mut terminal,
                &mut app_component,
                dispatcher.clone(),
                &runtime,
                worker_action_tx.clone(),
                client.clone(),
            )
        {
            tracing::event!(
                target: module_path!(),
                tracing::Level::ERROR,
                error = %err,
                "failed to handle app effect"
            );
        }
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
                    let size = terminal.size().expect("failed to get terminal size");
                    if !handle_key_event(
                        event,
                        &mut app_component,
                        dispatcher.clone(),
                        area_from_terminal_size(size.width, size.height),
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

fn area_from_terminal_size(width: u16, height: u16) -> Rect {
    Rect::new(0, 0, width, height)
}

fn handle_key_event(
    event: Event,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    area: Rect,
) -> bool {
    let should_continue = match event {
        Event::Key(_) => app_component.handle_key_event(event, dispatcher.clone()),
        _ => true,
    };
    update(dispatcher.clone(), app_component, area);
    app_component.update(dispatcher.clone(), dispatcher.borrow().store(), area);
    should_continue
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
        AppEffect::FetchIssue(id) => {
            start_issue_fetch(dispatcher, runtime, sender, client, id);
        }
        AppEffect::FetchProjectIssuesPage { project_id, page } => {
            start_project_issues_page_fetch(dispatcher, runtime, sender, client, project_id, page);
        }
        AppEffect::OpenEditor(request) => {
            let response = run_editor(terminal, request)?;
            app_component.handle_editor_response(response);
            let size = terminal.size().expect("failed to get terminal size");
            let rect = Rect::new(0, 0, size.width, size.height);
            app_component.update(dispatcher.clone(), dispatcher.borrow().store(), rect);
        }
        AppEffect::StartIssueUpload(id) => {
            let mut d = dispatcher.borrow_mut();
            if d.store().has_uploading_journal(id) {
                panic!("a journal of the issue is already uploading");
            }
            d.dispatch(IssueAction::StartUpload { id });
            let (_, state) = d
                .store()
                .get_issue(id)
                .expect("tried to upload unknown issue");
            if state != &IssueState::Edited {
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
        AppEffect::ContinueIssueUpload { id, diffs } => {
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

fn start_project_issues_page_fetch<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    runtime: &Runtime,
    sender: mpsc::Sender<Action>,
    client: Arc<C>,
    project_id: vos::ProjectId,
    page: std::num::NonZeroUsize,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let future = fetch_project_issues_page(dispatcher, client, project_id, page);
    runtime.spawn(async move {
        sender
            .send(future.await.into())
            .expect("Failed to send Action with mpsc::channel");
    });
}

fn start_issue_fetch<C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    runtime: &Runtime,
    sender: mpsc::Sender<Action>,
    client: Arc<C>,
    id: IssueId,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let Some(future) = fetch_issue(dispatcher, client, id) else {
        return;
    };

    runtime.spawn(async move {
        for action in future.await {
            sender
                .send(action)
                .expect("Failed to send Action with mpsc::channel");
        }
    });
}

async fn issue_upload_action(
    client: &impl RedmineClient,
    id: IssueId,
    diffs: &[IssuePropertyDiff],
) -> Action {
    let (mut server_issue, conflicts) = match fetch_issue_with_conflicts(client, id, diffs).await {
        Ok(result) => result,
        Err(_) => return IssueAction::FailUpload { id }.into(),
    };
    if !conflicts.is_empty() {
        return IssueAction::UploadConflictsDetected {
            server_issue,
            conflicts,
        }
        .into();
    }

    apply_issue_property_diffs(&mut server_issue, diffs);
    if upload_issue(client, &server_issue).await.is_err() {
        return IssueAction::FailUpload { id }.into();
    }

    IssueAction::Sync {
        issue: server_issue,
    }
    .into()
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
    d.dispatch(IssueAction::Load { id: 1.into() });
    d.dispatch(IssueAction::Load { id: 2.into() });
    d.dispatch(IssueAction::Load { id: 3.into() });
    d.dispatch(Action::LoadJournal { id: 1.into() });
    d.dispatch(Action::LoadJournal { id: 2.into() });
    d.dispatch(Action::LoadJournal { id: 3.into() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::{sync::Mutex, time::Duration};

    use crate::clients::redmine::{RedmineClient, RedmineClientError, RedmineHttpError};
    use crate::entities::{
        Category, Issue, IssueAggregate, IssueStatus, Journal, Priority, Project,
        ProjectIssuesPage, TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{JournalAction, ProjectIssuesAction};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crate::vos::{IssueId, IssuePropertyDiff, IssueStatusId, JournalId, JournalKey};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn area_from_terminal_size_uses_the_latest_dimensions() {
        assert_eq!(area_from_terminal_size(120, 40), Rect::new(0, 0, 120, 40));
    }

    #[test]
    fn key_event_updates_issue_popup_preview_even_when_it_dispatches_no_action() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher.borrow_mut());
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        let mut app = AppComponent::new(dispatcher.clone(), None);
        let Some(AppEffect::FetchProjectIssuesPage { project_id, page }) = app.take_effect() else {
            panic!("expected the initial project page effect")
        };
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        let request_id = crate::stores::ProjectIssuesRequestId::new();
        dispatcher
            .borrow_mut()
            .dispatch(ProjectIssuesAction::StartLoading {
                request_id,
                project_id,
                page,
            });
        dispatcher
            .borrow_mut()
            .dispatch(ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id,
                page,
                result: ProjectIssuesPage {
                    issues: vec![
                        Issue {
                            id: 41.into(),
                            project_id,
                            subject: "first issue".to_string(),
                            description: "first preview marker".to_string(),
                            status_id: 1.into(),
                        },
                        Issue {
                            id: 42.into(),
                            project_id,
                            subject: "second issue".to_string(),
                            description: "second preview marker".to_string(),
                            status_id: 1.into(),
                        },
                    ],
                    total_count: 2,
                    offset: 0,
                    limit: 50,
                },
            });
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));

        assert!(handle_key_event(
            Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('l'),
                crossterm::event::KeyModifiers::NONE,
            )),
            &mut app,
            dispatcher.clone(),
            Rect::new(0, 0, 80, 24),
        ));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert!(handle_key_event(
            Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('j'),
                crossterm::event::KeyModifiers::NONE,
            )),
            &mut app,
            dispatcher.clone(),
            Rect::new(0, 0, 80, 24),
        ));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| app.render(dispatcher.borrow().store(), frame, frame.area()))
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("second preview marker"));
        assert!(!rendered.contains("first preview marker"));
    }

    #[test]
    fn loop_update_takes_initial_fetch_effect_before_draw_and_routes_only_completion_to_worker_channel()
     {
        let runtime = init_tokio_runtime().unwrap();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut app = AppComponent::new(dispatcher.clone(), Some(42.into()));
        let mut fetched = sample_issue_aggregate(
            42,
            "fetched issue",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        );
        fetched.journal_keys = vec![JournalKey::Remote(JournalId::new(2))];
        let client = Arc::new(IssueUploadClient::with_journals(
            fetched,
            vec![parse_journal_yaml(JournalId::new(2))],
        ));
        let (sender, receiver) = mpsc::channel::<Action>();
        let completion_sender = sender.clone();

        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        let effect = app
            .take_effect()
            .expect("initial fetch effect should be taken before draw");
        let AppEffect::FetchIssue(id) = effect else {
            panic!("test app only has a fetch effect")
        };
        start_issue_fetch(dispatcher.clone(), &runtime, sender, client, id);

        let completions = (0..2)
            .map(|_| {
                receiver
                    .recv_timeout(Duration::from_secs(1))
                    .expect("fetch completion should be sent by the runtime task")
            })
            .collect::<Vec<_>>();
        for action in completions {
            completion_sender.send(action).unwrap();
        }
        move_worker_action(&receiver, dispatcher.clone());
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        let borrow = dispatcher.borrow();
        let (issue, state) = borrow.store().get_issue(42).unwrap();
        assert_eq!(state, &IssueState::Synced);
        assert_eq!(
            issue.journal_keys,
            vec![JournalKey::Remote(JournalId::new(2))]
        );
        assert!(
            borrow
                .store()
                .get_journal_entry(JournalKey::Remote(JournalId::new(2)))
                .is_some()
        );
    }

    #[test]
    fn worker_issue_fetch_failure_only_transitions_the_fetching_issue() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut app = AppComponent::new(dispatcher.clone(), Some(42.into()));
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartFetching { id: 42.into() });
        let (sender, receiver) = mpsc::channel();
        sender
            .send(
                IssueAction::FetchFailed {
                    id: 42.into(),
                    message: "offline".to_string(),
                }
                .into(),
            )
            .unwrap();

        move_worker_action(&receiver, dispatcher.clone());
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));

        assert_eq!(
            dispatcher.borrow().store().get_issue_state(42),
            Some(&IssueState::FetchFailed {
                message: "offline".to_string()
            })
        );
        assert!(
            !dispatcher
                .borrow()
                .store()
                .has_journal_entry_for_issue(42.into())
        );
    }

    #[test]
    fn start_issue_upload_panics_without_dispatch_or_worker_when_a_journal_is_uploading() {
        let runtime = init_tokio_runtime().unwrap();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut aggregate =
            sample_issue_aggregate(42, "subject", IssueStatusId::new(1), None, None, None, 0);
        aggregate.journal_keys = vec![JournalKey::Remote(JournalId::new(1))];
        let mut d = dispatcher.borrow_mut();
        d.dispatch(IssueAction::Sync { issue: aggregate });
        d.consume_action();
        d.dispatch(JournalAction::RegisterRemote {
            journal: parse_journal_yaml(JournalId::new(1)),
            issue_id: 42.into(),
        });
        d.consume_action();
        d.dispatch(JournalAction::EditRemoteNotes {
            id: 1.into(),
            notes: "edited".to_string(),
        });
        d.consume_action();
        d.dispatch(JournalAction::StartUpload {
            key: JournalKey::Remote(JournalId::new(1)),
        });
        d.consume_action();
        drop(d);

        let client = Arc::new(DefaultRedmineClient::new("http://127.0.0.1:8080", "token"));
        let (sender, _receiver) = mpsc::channel::<Action>();
        let mut terminal =
            Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout())).unwrap();
        let mut app = AppComponent::new(dispatcher.clone(), Some(42.into()));

        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                handle_app_effect(
                    AppEffect::StartIssueUpload(42.into()),
                    &mut terminal,
                    &mut app,
                    dispatcher.clone(),
                    &runtime,
                    sender,
                    client,
                )
            }))
            .is_err()
        );
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(
            dispatcher.borrow().store().get_issue_state(42),
            Some(&IssueState::Synced)
        );
    }

    #[test]
    fn project_page_effect_queues_start_loading_and_routes_only_completion_to_worker_channel() {
        let runtime = init_tokio_runtime().unwrap();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher.borrow_mut());
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        let mut app = AppComponent::new(dispatcher.clone(), None);
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        let client = Arc::new(IssueUploadClient::new(sample_issue_aggregate(
            1,
            "unused",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        )));
        let (sender, receiver) = mpsc::channel::<Action>();
        let effect = app
            .take_effect()
            .expect("initial popup effect should be taken at the common loop point");
        let AppEffect::FetchProjectIssuesPage { project_id, page } = effect else {
            panic!("initial popup should request a project issue page")
        };

        start_project_issues_page_fetch(
            dispatcher.clone(),
            &runtime,
            sender,
            client,
            project_id,
            page,
        );

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(1, std::num::NonZeroUsize::MIN)
                .is_none()
        );
        let completion = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("page completion should be sent by the runtime task");
        assert!(matches!(
            completion,
            Action::ProjectIssues(stores::ProjectIssuesAction::LoadSucceeded {
                project_id,
                page,
                ..
            }) if project_id == vos::ProjectId::new(1)
                && page == std::num::NonZeroUsize::MIN
        ));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
    }

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
        let mut server_issue = sample_issue_aggregate(
            1,
            "server subject",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        );
        server_issue.updated_on = crate::test_support::local_datetime("2026-08-23T12:00:00+09:00");
        server_issue.issue.description = "original description".to_string();
        let client = IssueUploadClient::new(server_issue.clone());
        let diffs = vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "original description".to_string(),
            after: "local description".to_string(),
        })];

        let action = issue_upload_action(&client, 1.into(), &diffs).await;

        let Action::Issue(IssueAction::Sync { issue }) = action else {
            panic!("expected Sync");
        };
        assert_eq!(issue.issue.subject, "server subject");
        assert_eq!(issue.updated_on, server_issue.updated_on);
        assert_eq!(issue.issue.description, "local description");
        let uploaded = client.uploaded.lock().unwrap();
        assert_eq!(uploaded.len(), 1);
        assert_eq!(uploaded[0].issue.subject, issue.issue.subject);
        assert_eq!(uploaded[0].issue.description, issue.issue.description);
        assert_eq!(uploaded[0].updated_on, issue.updated_on);
    }

    #[tokio::test]
    async fn issue_upload_returns_fail_action_when_fetch_fails() {
        let client = IssueUploadClient::failing_get();

        let action = issue_upload_action(&client, 1.into(), &[]).await;

        assert!(matches!(
            action,
            Action::Issue(IssueAction::FailUpload { id }) if id == IssueId::new(1)
        ));
        assert!(client.uploaded.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn issue_upload_returns_fail_action_when_update_fails() {
        let server_issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        let client = IssueUploadClient::failing_update(server_issue);

        let action = issue_upload_action(&client, 1.into(), &[]).await;

        assert!(matches!(
            action,
            Action::Issue(IssueAction::FailUpload { id }) if id == IssueId::new(1)
        ));
    }

    #[tokio::test]
    async fn issue_upload_returns_conflict_action_when_property_conflicts() {
        let mut server_issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        server_issue.issue.description = "server description".to_string();
        let client = IssueUploadClient::new(server_issue);
        let diffs = vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
            before: "original description".to_string(),
            after: "local description".to_string(),
        })];

        let action = issue_upload_action(&client, 1.into(), &diffs).await;

        let Action::Issue(IssueAction::UploadConflictsDetected {
            server_issue,
            conflicts,
        }) = action
        else {
            panic!("expected UploadConflictsDetected");
        };
        assert_eq!(server_issue.issue.description, "server description");
        assert_eq!(conflicts, diffs);
        assert!(client.uploaded.lock().unwrap().is_empty());
    }

    struct IssueUploadClient {
        issue: Option<IssueAggregate>,
        journals: Vec<Journal>,
        get_error: bool,
        update_error: bool,
        uploaded: Mutex<Vec<IssueAggregate>>,
    }

    impl IssueUploadClient {
        fn new(issue: IssueAggregate) -> Self {
            Self::with_journals(issue, Vec::new())
        }

        fn with_journals(issue: IssueAggregate, journals: Vec<Journal>) -> Self {
            Self {
                issue: Some(issue),
                journals,
                get_error: false,
                update_error: false,
                uploaded: Mutex::new(Vec::new()),
            }
        }

        fn failing_get() -> Self {
            Self {
                issue: None,
                journals: Vec::new(),
                get_error: true,
                update_error: false,
                uploaded: Mutex::new(Vec::new()),
            }
        }

        fn failing_update(issue: IssueAggregate) -> Self {
            Self {
                issue: Some(issue),
                journals: Vec::new(),
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
        async fn get_issue(
            &self,
            _: IssueId,
        ) -> std::result::Result<(IssueAggregate, Vec<crate::entities::Journal>), RedmineClientError>
        {
            if self.get_error {
                return Err(Self::network_error());
            }
            Ok((
                self.issue.clone().expect("test issue must exist"),
                self.journals.clone(),
            ))
        }

        async fn update_issue(
            &self,
            issue: &IssueAggregate,
        ) -> std::result::Result<(), RedmineClientError> {
            if self.update_error {
                return Err(Self::network_error());
            }
            self.uploaded.lock().unwrap().push(issue.clone());
            Ok(())
        }

        async fn create_journal(
            &self,
            _: crate::vos::IssueId,
            _: &str,
        ) -> std::result::Result<(), RedmineClientError> {
            unreachable!()
        }

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            _: &str,
        ) -> std::result::Result<(), RedmineClientError> {
            unreachable!()
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

        async fn get_project_issues(
            &self,
            _: crate::vos::ProjectId,
            _: std::num::NonZeroUsize,
        ) -> std::result::Result<crate::entities::ProjectIssuesPage, RedmineClientError> {
            Ok(crate::entities::ProjectIssuesPage {
                issues: Vec::new(),
                total_count: 0,
                offset: 0,
                limit: 50,
            })
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

        async fn get_issue(
            &self,
            _: IssueId,
        ) -> std::result::Result<(IssueAggregate, Vec<crate::entities::Journal>), RedmineClientError>
        {
            Err(self.unauthorized())
        }

        async fn update_issue(
            &self,
            _: &IssueAggregate,
        ) -> std::result::Result<(), RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn create_journal(
            &self,
            _: crate::vos::IssueId,
            _: &str,
        ) -> std::result::Result<(), RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            _: &str,
        ) -> std::result::Result<(), RedmineClientError> {
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

        async fn get_project_issues(
            &self,
            _: crate::vos::ProjectId,
            _: std::num::NonZeroUsize,
        ) -> std::result::Result<crate::entities::ProjectIssuesPage, RedmineClientError> {
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
