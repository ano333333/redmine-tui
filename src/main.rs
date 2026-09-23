mod clients;
mod components;
mod entities;
mod libs;
mod logging;
mod platform;
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
use ratatui::{Frame, layout::Rect};
use std::{cell::RefCell, env, io, process::ExitCode, rc::Rc, sync::Arc};

use self::{
    clients::redmine::{DefaultRedmineClient, RedmineClient},
    components::{AppComponent, app::AppEffect},
    platform::editor::{EditorOutcome, TextEditor, native::NativeTextEditor},
    platform::host::native::{
        install_panic_hook, run_terminal_operations, run_terminal_operations_with_rollback,
    },
    platform::input::native::convert_key,
    platform::runtime::{
        BackgroundCompletion, BackgroundSpawner, LocalTask, tokio_spawner::TokioBackgroundSpawner,
    },
    stores::{Action, Dispatcher, NoticeAction, NoticeId},
    usecases::redmine::{
        continue_remote_journal_upload, fetch_issue, fetch_project_issues_page,
        load_initial_entities, start_issue_upload, start_local_journal_upload,
        start_remote_journal_upload, upload_issue_action,
    },
    vos::{IssueId, JournalId},
};

const TICK_RATE_MS: u64 = 250;
const REDMINE_API_KEY_ENV: &str = "REDMINE_API_KEY";
const REDMINE_URL_ENV: &str = "REDMINE_URL";
const REDMINE_PORT_ENV: &str = "REDMINE_PORT";

type EditorSession<'a> = LocalTask<'a, io::Result<EditorOutcome>>;

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
    let mut terminal = ratatui::init();
    // 以降のどのreturnでもterminalを復帰する。local変数は宣言の逆順にdropされるため、
    // editor sessionを先に停止できるようこのguardはそれより前に宣言する。
    let _terminal_restore = TerminalRestoreGuard;
    let mut app_component = AppComponent::new(dispatcher.clone(), None);
    app_component.update(
        dispatcher.clone(),
        dispatcher.borrow().store(),
        terminal.get_frame().area(),
    );
    let tick_rate = std::time::Duration::from_millis(TICK_RATE_MS);
    let mut last_tick = std::time::Instant::now();
    let editor = NativeTextEditor::from_environment();
    let mut editor_session: Option<EditorSession<'_>> = None;
    loop {
        // editor中もworker完了はStoreへ取り込むが、Component更新・描画・入力と
        // Noticeの経過時間更新はeditor終了まで遅延する。
        if let Some(session) = editor_session.as_mut() {
            if let Some(message) = consume_editor_worker_actions(&spawner, dispatcher.clone()) {
                eprintln!("worker task panicked: {message}");
                return ExitCode::FAILURE;
            }
            let outcome = match session.poll_completion() {
                None => {
                    std::thread::sleep(tick_rate);
                    continue;
                }
                Some(outcome) => outcome,
            };
            editor_session = None;
            let mut enter_alternate_screen = || execute!(std::io::stdout(), EnterAlternateScreen);
            let mut enable_raw = || enable_raw_mode();
            let mut clear_terminal = || terminal.clear();
            let terminal_result = run_terminal_operations(&mut [
                &mut enter_alternate_screen,
                &mut enable_raw,
                &mut clear_terminal,
            ]);
            match terminal_result {
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
            let size = terminal.size().expect("failed to get terminal size");
            app_component.update(
                dispatcher.clone(),
                dispatcher.borrow().store(),
                area_from_terminal_size(size.width, size.height),
            );
            // editor滞在時間を次のNotice tickへ混ぜないよう、通常loopへ戻る前にresetする。
            last_tick = std::time::Instant::now();
        }
        if let Some(message) = move_worker_action(&spawner, dispatcher.clone()) {
            eprintln!("worker task panicked: {message}");
            return ExitCode::FAILURE;
        }
        let now = std::time::Instant::now();
        let tick = tick_since(last_tick, now);
        last_tick = now;
        dispatcher.borrow_mut().update_store(tick);
        let size = terminal.size().expect("failed to get terminal size");
        update(
            dispatcher.clone(),
            &mut app_component,
            area_from_terminal_size(size.width, size.height),
        );
        let effect = app_component.take_effect();
        if let Some(effect) = effect {
            handle_app_effect(
                effect,
                &mut app_component,
                dispatcher.clone(),
                &spawner,
                client.clone(),
                &editor,
                &mut editor_session,
            );
        }
        if editor_session.is_some() {
            continue;
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
    trace_dbg!("done");
    ExitCode::SUCCESS
}

/// `now`が`last`より前でないことを前提とし、`chrono::Duration`の範囲外ならゼロを返す。
fn tick_since(last: std::time::Instant, now: std::time::Instant) -> chrono::Duration {
    chrono::Duration::from_std(now.duration_since(last))
        .unwrap_or_else(|_| chrono::Duration::zero())
}

fn move_worker_action<S: BackgroundSpawner>(
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

fn consume_editor_worker_actions<S: BackgroundSpawner>(
    spawner: &S,
    dispatcher: Rc<RefCell<Dispatcher>>,
) -> Option<String> {
    let worker_panic_message = move_worker_action(spawner, dispatcher.clone());
    while dispatcher.borrow().consume_actinos_len() > 0 {
        dispatcher.borrow_mut().consume_action();
    }
    worker_panic_message
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
        Event::Key(key) => convert_key(key).map_or(true, |event| {
            app_component.handle_key_event(event, dispatcher.clone())
        }),
        _ => true,
    };
    update(dispatcher.clone(), app_component, area);
    app_component.update(dispatcher.clone(), dispatcher.borrow().store(), area);
    should_continue
}

fn handle_app_effect<'a, S: BackgroundSpawner>(
    effect: AppEffect,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<DefaultRedmineClient>,
    editor: &'a NativeTextEditor,
    editor_session: &mut Option<EditorSession<'a>>,
) {
    match effect {
        AppEffect::FetchIssue(id) => {
            start_issue_fetch(dispatcher, spawner, client, id);
        }
        AppEffect::FetchProjectIssuesPage { project_id, page } => {
            start_project_issues_page_fetch(dispatcher, spawner, client, project_id, page);
        }
        AppEffect::OpenEditor(request) => {
            // FIXME: 実terminalとexternal editor processを使い、editorの成否にかかわらず長時間滞在後もNoticeが残ることをE2E testで確認する。
            // FIXME: 実terminalとexternal editor processを使い、editor失敗時のnotice追加とfocus/cursor維持をE2E testで確認する。
            // FIXME: 実terminalとexternal editor processを使い、アプリ終了時にterminal状態が復元されeditor processがkillされることをE2E testで確認する。
            let mut disable_raw = || disable_raw_mode();
            let mut leave_alternate_screen = || execute!(std::io::stdout(), LeaveAlternateScreen);
            let mut enable_raw = || enable_raw_mode();
            let mut enter_alternate_screen = || execute!(std::io::stdout(), EnterAlternateScreen);
            if let Err(error) = run_terminal_operations_with_rollback(&mut [
                (&mut disable_raw, &mut enable_raw),
                (&mut leave_alternate_screen, &mut enter_alternate_screen),
            ]) {
                handle_editor_failure(
                    app_component,
                    dispatcher,
                    &error,
                    "failed to leave terminal for editor",
                );
            } else {
                *editor_session = Some(LocalTask::new(editor.edit(request)));
            }
        }
        AppEffect::StartIssueUpload(id) => {
            let future = start_issue_upload(dispatcher, client, id);
            spawner.spawn(future);
        }
        AppEffect::ContinueIssueUpload { id, diffs } => {
            spawner.spawn(async move { upload_issue_action(client.as_ref(), id, &diffs).await });
        }
        AppEffect::StartRemoteJournalUpload {
            issue_id,
            journal_id,
        } => {
            start_remote_journal_upload_action(dispatcher, spawner, client, issue_id, journal_id);
        }
        AppEffect::StartLocalJournalUpload { issue_id } => {
            start_local_journal_upload_action(dispatcher, spawner, client, issue_id);
        }
        AppEffect::ContinueRemoteJournalUpload {
            issue_id,
            journal_id,
            resolved_notes,
        } => {
            continue_remote_journal_upload_action(
                dispatcher,
                spawner,
                client,
                issue_id,
                journal_id,
                resolved_notes,
            );
        }
    }
}

fn handle_editor_failure(
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    error: &dyn std::fmt::Display,
    message: &'static str,
) {
    app_component.handle_editor_response(EditorOutcome::Failed);
    tracing::event!(target: module_path!(), tracing::Level::ERROR, error = %error, "{message}");
    dispatcher
        .borrow_mut()
        .dispatch(editor_failure_notice_action(error));
}

fn start_remote_journal_upload_action<S: BackgroundSpawner, C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    issue_id: IssueId,
    journal_id: JournalId,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let future = start_remote_journal_upload(dispatcher, client, issue_id, journal_id);
    spawner.spawn(future);
}

fn start_local_journal_upload_action<S: BackgroundSpawner, C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    issue_id: IssueId,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let future = start_local_journal_upload(dispatcher, client, issue_id);
    spawner.spawn(future);
}

fn continue_remote_journal_upload_action<S: BackgroundSpawner, C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    issue_id: IssueId,
    journal_id: JournalId,
    resolved_notes: String,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let future =
        continue_remote_journal_upload(dispatcher, client, issue_id, journal_id, resolved_notes);
    spawner.spawn(future);
}

fn start_project_issues_page_fetch<S: BackgroundSpawner, C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    project_id: vos::ProjectId,
    page: std::num::NonZeroUsize,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let future = fetch_project_issues_page(dispatcher, client, project_id, page);
    spawner.spawn(async move { vec![future.await.into()] });
}

fn start_issue_fetch<S: BackgroundSpawner, C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    id: IssueId,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let Some(future) = fetch_issue(dispatcher, client, id) else {
        return;
    };

    // Issueが取得済みになる前にJournalを同期するため、usecaseが定めた順序を維持する。
    spawner.spawn(future);
}

fn editor_failure_notice_action(error: &dyn std::fmt::Display) -> NoticeAction {
    NoticeAction::Push {
        id: NoticeId::new(),
        message: format!("エディタによる編集に失敗しました: {error}"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};
    use std::{
        collections::VecDeque,
        future::Future,
        sync::Mutex,
        time::{Duration, Instant},
    };

    use crate::clients::redmine::base::FetchedIssue;
    use crate::clients::redmine::{RedmineClient, RedmineClientError, RedmineHttpError};
    use crate::entities::{
        Category, Issue, IssueAggregate, IssueStatus, Journal, Priority, Project,
        ProjectIssuesPage, TargetVersion, TimeEntityActivity, Tracker, User,
    };
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{IssueAction, JournalAction, NoticeAction, NoticeId, ProjectIssuesAction};
    use crate::test_support::sample_issue_aggregate;
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crate::vos::{IssueId, IssuePropertyDiff, IssueStatusId};
    use ratatui::{Terminal, backend::TestBackend, widgets::Widget};

    fn recv_completion(spawner: &TokioBackgroundSpawner) -> BackgroundCompletion {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(completion) = spawner.try_recv_completion() {
                return completion;
            }
            assert!(
                Instant::now() < deadline,
                "completion did not arrive in time"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn recv_actions(spawner: &TokioBackgroundSpawner, expected: usize) -> Vec<Action> {
        let mut actions = Vec::new();
        while actions.len() < expected {
            match recv_completion(spawner) {
                BackgroundCompletion::Succeeded(mut completed) => actions.append(&mut completed),
                BackgroundCompletion::Panicked { message } => {
                    panic!("worker task panicked: {message}")
                }
            }
        }
        actions
    }

    struct CompletionSpawner {
        completions: RefCell<VecDeque<BackgroundCompletion>>,
    }

    impl CompletionSpawner {
        fn new(actions: Vec<Action>) -> Self {
            Self::from_completions(vec![BackgroundCompletion::Succeeded(actions)])
        }

        fn from_completions(completions: Vec<BackgroundCompletion>) -> Self {
            Self {
                completions: RefCell::new(completions.into_iter().collect()),
            }
        }

        fn panicked(message: &str) -> BackgroundCompletion {
            BackgroundCompletion::Panicked {
                message: message.to_string(),
            }
        }
    }

    impl BackgroundSpawner for CompletionSpawner {
        fn spawn<F>(&self, _: F)
        where
            F: Future<Output = Vec<Action>> + Send + 'static,
        {
        }

        fn try_recv_completion(&self) -> Option<BackgroundCompletion> {
            self.completions.borrow_mut().pop_front()
        }
    }

    #[test]
    fn area_from_terminal_size_uses_the_latest_dimensions() {
        assert_eq!(area_from_terminal_size(120, 40), Rect::new(0, 0, 120, 40));
    }

    #[test]
    fn tick_since_returns_zero_for_the_same_instant() {
        let now = std::time::Instant::now();

        assert_eq!(tick_since(now, now), chrono::Duration::zero());
    }

    #[test]
    fn tick_since_returns_a_positive_duration_after_elapsed_time() {
        let last = std::time::Instant::now();
        std::thread::sleep(Duration::from_millis(10));
        let now = std::time::Instant::now();

        assert!(tick_since(last, now) > chrono::Duration::zero());
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
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut app = AppComponent::new(dispatcher.clone(), Some(42.into()));
        let client = Arc::new(IssueUploadClient::new(sample_issue_aggregate(
            42,
            "fetched issue",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        )));

        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        let effect = app
            .take_effect()
            .expect("initial fetch effect should be taken before draw");
        let AppEffect::FetchIssue(id) = effect else {
            panic!("test app only has a fetch effect")
        };
        start_issue_fetch(dispatcher.clone(), &spawner, client, id);

        assert!(app.take_effect().is_none());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert_eq!(dispatcher.borrow().store().try_get_issue_state(42), None);
        assert_eq!(
            dispatcher.borrow().store().try_get_issue_fetch_state(42),
            None
        );
        let mut actions = recv_actions(&spawner, 2).into_iter();
        let first_completion = actions.next().expect("first completion");
        assert!(matches!(
            &first_completion,
            Action::Journal(JournalAction::SyncFetched { issue_id, journals })
                if *issue_id == IssueId::new(42) && journals.is_empty()
        ));
        let completion = actions.next().expect("second completion");
        assert!(matches!(
            &completion,
            Action::Issue(IssueAction::FetchSucceeded { id, issue })
                if *id == IssueId::new(42) && issue.issue.id == IssueId::new(42)
        ));
        assert_eq!(
            dispatcher.borrow().consume_actinos_len(),
            1,
            "the spawned future must not dispatch or consume actions itself"
        );
        for action in [first_completion, completion] {
            dispatcher.borrow_mut().dispatch(action);
        }
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        assert!(matches!(
            dispatcher.borrow().store().try_get_issue_state(42),
            Some(crate::stores::IssueState::Synced)
        ));
    }

    #[test]
    fn issue_detail_shows_journals_from_the_first_frame_after_ordered_fetch_actions() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut d = dispatcher.borrow_mut();
            crate::test_support::dispatch_fixture_entity_actions(&mut d);
            d.dispatch(IssueAction::StartFetching { id: 42.into() });
            d.dispatch(Action::Journal(JournalAction::SyncFetched {
                issue_id: 42.into(),
                journals: vec![sample_journal(42)],
            }));
            d.dispatch(IssueAction::FetchSucceeded {
                id: 42.into(),
                issue: sample_issue_aggregate(
                    42,
                    "subject",
                    IssueStatusId::new(1),
                    None,
                    None,
                    None,
                    0,
                ),
            });
            while d.consume_actinos_len() > 0 {
                d.consume_action();
            }
        }

        let mut component = crate::components::issue::IssueDetailComponent::new(42);
        let frame_area = Rect::new(0, 0, 80, 100);
        component.update(dispatcher.clone(), dispatcher.borrow().store(), (80, 100));
        let d = dispatcher.borrow();
        let store = d.store();
        let widget = component.create_widget(store);
        let mut buffer = ratatui::buffer::Buffer::empty(frame_area);
        widget.render(frame_area, &mut buffer);
        let rendered = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("first journal notes marker"));
    }

    #[test]
    fn project_page_effect_queues_start_loading_and_routes_only_completion_to_worker_channel() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
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
        let effect = app
            .take_effect()
            .expect("initial popup effect should be taken at the common loop point");
        let AppEffect::FetchProjectIssuesPage { project_id, page } = effect else {
            panic!("initial popup should request a project issue page")
        };

        start_project_issues_page_fetch(dispatcher.clone(), &spawner, client, project_id, page);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        assert!(
            dispatcher
                .borrow()
                .store()
                .get_project_issues_page_state(1, std::num::NonZeroUsize::MIN)
                .is_none()
        );
        let completion = recv_actions(&spawner, 1).remove(0);
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
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let client = FailingClient;

        let error = match spawner.block_on(load_initial_entities(&client)) {
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
        let client = Arc::new(IssueUploadClient::new(server_issue.clone()));
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher.borrow_mut().dispatch(IssueAction::Sync {
            issue: server_issue.clone(),
        });
        dispatcher.borrow_mut().consume_action();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateDescription {
                id: 1.into(),
                body: "local description".to_string(),
            });
        dispatcher.borrow_mut().consume_action();

        let actions = start_issue_upload(dispatcher.clone(), client.clone(), 1.into()).await;

        assert_eq!(actions.len(), 1);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        let [Action::Issue(IssueAction::Sync { issue })] = actions.as_slice() else {
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

    #[test]
    #[should_panic(expected = "uploading issue is not edited")]
    fn starting_issue_upload_panics_when_issue_is_not_edited() {
        let issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        let client = Arc::new(IssueUploadClient::new(issue.clone()));
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::Sync { issue });
        dispatcher.borrow_mut().consume_action();

        std::mem::drop(start_issue_upload(dispatcher, client, 1.into()));
    }

    #[tokio::test]
    async fn issue_upload_returns_fail_action_when_fetch_fails() {
        let client = IssueUploadClient::failing_get();

        let actions = upload_issue_action(&client, 1.into(), &[]).await;

        assert_eq!(actions.len(), 2);
        let Action::Notice(NoticeAction::Push { message, .. }) = &actions[0] else {
            panic!("expected failure notice action");
        };
        assert_eq!(
            message,
            "Issue #1の保存に失敗しました: network error: offline"
        );
        let Action::Issue(IssueAction::FailUpload { id, message }) = &actions[1] else {
            panic!("expected fail upload action");
        };
        assert_eq!(*id, IssueId::new(1));
        assert_eq!(message, "network error: offline");
        assert!(client.uploaded.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn issue_upload_returns_fail_action_when_update_fails() {
        let server_issue =
            sample_issue_aggregate(1, "subject", IssueStatusId::new(1), None, None, None, 0);
        let client = IssueUploadClient::failing_update(server_issue);

        let actions = upload_issue_action(&client, 1.into(), &[]).await;

        assert_eq!(actions.len(), 2);
        let Action::Notice(NoticeAction::Push { message, .. }) = &actions[0] else {
            panic!("expected failure notice action");
        };
        assert_eq!(
            message,
            "Issue #1の保存に失敗しました: network error: offline"
        );
        let Action::Issue(IssueAction::FailUpload { id, message }) = &actions[1] else {
            panic!("expected fail upload action");
        };
        assert_eq!(*id, IssueId::new(1));
        assert_eq!(message, "network error: offline");
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

        let actions = upload_issue_action(&client, 1.into(), &diffs).await;

        assert_eq!(actions.len(), 1);
        let [
            Action::Issue(IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
            }),
        ] = actions.as_slice()
        else {
            panic!("expected UploadConflictsDetected");
        };
        assert_eq!(server_issue.issue.description, "server description");
        assert_eq!(conflicts, &diffs);
        assert!(client.uploaded.lock().unwrap().is_empty());
    }

    #[test]
    fn background_completion_panic_is_reported_by_the_main_loop_acceptor() {
        let spawner = CompletionSpawner::from_completions(vec![
            BackgroundCompletion::Succeeded(vec![
                NoticeAction::Push {
                    id: NoticeId::new(),
                    message: "completed before panic".to_string(),
                }
                .into(),
            ]),
            CompletionSpawner::panicked("worker panic marker"),
        ]);
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));

        let message = move_worker_action(&spawner, dispatcher.clone());

        assert_eq!(message.as_deref(), Some("worker panic marker"));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(dispatcher.borrow().store().get_notices().len(), 1);
    }

    #[test]
    fn update_store_removes_expired_notices() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        for _ in 0..2 {
            dispatcher.borrow_mut().dispatch(NoticeAction::Push {
                id: NoticeId::new(),
                message: "failed".to_string(),
            });
        }
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        assert_eq!(dispatcher.borrow().store().get_notices().len(), 2);

        dispatcher
            .borrow_mut()
            .update_store(chrono::Duration::seconds(5));

        assert!(dispatcher.borrow().store().get_notices().is_empty());
    }

    #[test]
    fn update_store_keeps_notices_within_the_visible_duration() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher.borrow_mut().dispatch(NoticeAction::Push {
            id: NoticeId::new(),
            message: "fresh".to_string(),
        });
        dispatcher.borrow_mut().consume_action();

        dispatcher
            .borrow_mut()
            .update_store(chrono::Duration::seconds(1));

        assert_eq!(dispatcher.borrow().store().get_notices().len(), 1);
    }

    struct IssueUploadClient {
        issue: Option<IssueAggregate>,
        journals: Vec<Journal>,
        get_error: bool,
        update_error: bool,
        update_journal_result: std::result::Result<(), RedmineClientError>,
        update_issue_notes_result: std::result::Result<(), RedmineClientError>,
        uploaded_journal_notes: Mutex<Vec<String>>,
        uploaded_issue_notes: Mutex<Vec<String>>,
        uploaded: Mutex<Vec<IssueAggregate>>,
        // リトライを同じclientで検証できるよう、指定回数だけ通信失敗を返し、
        // カウンタが0になった後は通常のレスポンスへ戻す。
        get_failures_remaining: Mutex<usize>,
        journal_failures_remaining: Mutex<usize>,
        local_failures_remaining: Mutex<usize>,
        get_requests: Mutex<usize>,
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
                update_journal_result: Ok(()),
                update_issue_notes_result: Ok(()),
                uploaded_journal_notes: Mutex::new(Vec::new()),
                uploaded_issue_notes: Mutex::new(Vec::new()),
                uploaded: Mutex::new(Vec::new()),
                get_failures_remaining: Mutex::new(0),
                journal_failures_remaining: Mutex::new(0),
                local_failures_remaining: Mutex::new(0),
                get_requests: Mutex::new(0),
            }
        }

        fn failing_get() -> Self {
            Self {
                issue: None,
                journals: Vec::new(),
                get_error: true,
                update_error: false,
                update_journal_result: Ok(()),
                update_issue_notes_result: Ok(()),
                uploaded_journal_notes: Mutex::new(Vec::new()),
                uploaded_issue_notes: Mutex::new(Vec::new()),
                uploaded: Mutex::new(Vec::new()),
                get_failures_remaining: Mutex::new(0),
                journal_failures_remaining: Mutex::new(0),
                local_failures_remaining: Mutex::new(0),
                get_requests: Mutex::new(0),
            }
        }

        fn failing_update(issue: IssueAggregate) -> Self {
            Self {
                issue: Some(issue),
                journals: Vec::new(),
                get_error: false,
                update_error: true,
                update_journal_result: Ok(()),
                update_issue_notes_result: Ok(()),
                uploaded_journal_notes: Mutex::new(Vec::new()),
                uploaded_issue_notes: Mutex::new(Vec::new()),
                uploaded: Mutex::new(Vec::new()),
                get_failures_remaining: Mutex::new(0),
                journal_failures_remaining: Mutex::new(0),
                local_failures_remaining: Mutex::new(0),
                get_requests: Mutex::new(0),
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
        ) -> std::result::Result<FetchedIssue, RedmineClientError> {
            *self.get_requests.lock().unwrap() += 1;
            let mut failures_remaining = self.get_failures_remaining.lock().unwrap();
            if *failures_remaining > 0 {
                *failures_remaining -= 1;
                return Err(Self::network_error());
            }
            if self.get_error {
                return Err(Self::network_error());
            }
            Ok(FetchedIssue {
                aggregate: self.issue.clone().expect("test issue must exist"),
                journals: self.journals.clone(),
            })
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

        async fn update_journal_notes(
            &self,
            _: crate::vos::JournalId,
            notes: &str,
        ) -> std::result::Result<(), RedmineClientError> {
            self.uploaded_journal_notes
                .lock()
                .unwrap()
                .push(notes.to_string());
            let mut failures_remaining = self.journal_failures_remaining.lock().unwrap();
            if *failures_remaining > 0 {
                *failures_remaining -= 1;
                return Err(Self::network_error());
            }
            self.update_journal_result.clone()
        }

        async fn update_issue_notes(
            &self,
            _: crate::vos::IssueId,
            notes: &str,
        ) -> std::result::Result<(), RedmineClientError> {
            self.uploaded_issue_notes
                .lock()
                .unwrap()
                .push(notes.to_string());
            let mut failures_remaining = self.local_failures_remaining.lock().unwrap();
            if *failures_remaining > 0 {
                *failures_remaining -= 1;
                return Err(Self::network_error());
            }
            self.update_issue_notes_result.clone()
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

    fn sample_journal(issue_id: u16) -> Journal {
        Journal {
            id: JournalId::new(1),
            issue_id: IssueId::new(issue_id),
            user: "alice".to_string(),
            updated_on: Some(crate::test_support::local_datetime(
                "2026-01-15T00:00:00+09:00",
            )),
            details: vec![],
            notes: "first journal notes marker".to_string(),
        }
    }

    fn expect_string_error<T>(result: std::result::Result<T, String>) -> String {
        match result {
            Ok(_) => panic!("redmine connection config read succeeded"),
            Err(error) => error,
        }
    }

    fn start_edited_journal_upload(dispatcher: &mut Dispatcher, issue_id: u16, notes: &str) {
        let mut issue = sample_issue_aggregate(
            issue_id,
            "subject",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        );
        issue.issue.description = "issue body".to_string();
        dispatcher.dispatch(Action::Issue(IssueAction::Sync { issue }));
        dispatcher.consume_action();
        let mut journal = sample_journal(issue_id);
        journal.notes = notes.to_string();
        dispatcher.dispatch(Action::Journal(JournalAction::SyncFetched {
            issue_id: IssueId::new(issue_id),
            journals: vec![journal],
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
            issue_id: IssueId::new(issue_id),
            journal_id: JournalId::new(1),
            notes: "edited notes".to_string(),
        }));
        dispatcher.consume_action();
    }

    #[test]
    fn move_worker_action_dispatches_worker_actions_without_extra_notice() {
        let mut dispatcher = Dispatcher::new();
        start_edited_journal_upload(&mut dispatcher, 3, "first journal notes marker");
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: IssueId::new(3),
            journal_id: JournalId::new(1),
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let spawner =
            CompletionSpawner::new(vec![Action::Journal(JournalAction::FailRemoteUpload {
                issue_id: IssueId::new(3),
                journal_id: JournalId::new(1),
                message: "network error: offline".to_string(),
            })]);

        let panic_message = move_worker_action(&spawner, dispatcher.clone());

        assert!(panic_message.is_none());
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        let dispatcher = dispatcher.borrow();
        let store = dispatcher.store();
        assert!(store.get_notices().is_empty());
        match &store
            .get_remote_journal(IssueId::new(3), JournalId::new(1))
            .state
        {
            crate::stores::RemoteJournalState::Edited { failure, .. } => {
                assert_eq!(
                    failure.as_ref().unwrap().message.as_str(),
                    "network error: offline"
                );
            }
            other => panic!("expected edited state, got {other:?}"),
        }
    }

    #[test]
    fn move_worker_action_dispatches_no_notice_for_complete_remote_upload() {
        let mut dispatcher = Dispatcher::new();
        start_edited_journal_upload(&mut dispatcher, 3, "first journal notes marker");
        dispatcher.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
            issue_id: IssueId::new(3),
            journal_id: JournalId::new(1),
        }));
        dispatcher.consume_action();
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let spawner =
            CompletionSpawner::new(vec![Action::Journal(JournalAction::CompleteRemoteUpload {
                issue_id: IssueId::new(3),
                journal_id: JournalId::new(1),
                notes: "edited notes".to_string(),
            })]);

        let panic_message = move_worker_action(&spawner, dispatcher.clone());

        assert!(panic_message.is_none());
        while dispatcher.borrow().consume_actinos_len() > 0 {
            dispatcher.borrow_mut().consume_action();
        }
        assert!(dispatcher.borrow().store().get_notices().is_empty());
        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_remote_journal(IssueId::new(3), JournalId::new(1))
                .state,
            crate::stores::RemoteJournalState::Synced
        ));
    }

    #[test]
    fn editor_worker_actions_are_consumed_without_component_updates() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let spawner = CompletionSpawner::new(vec![
            NoticeAction::Push {
                id: NoticeId::new(),
                message: "completed while editing".to_string(),
            }
            .into(),
        ]);

        let panic_message = consume_editor_worker_actions(&spawner, dispatcher.clone());

        assert!(panic_message.is_none());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(dispatcher.borrow().store().get_notices().len(), 1);
    }

    #[test]
    fn start_remote_journal_upload_action_routes_the_upload_completion_to_worker_channel() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let mut dispatcher = Dispatcher::new();
        start_edited_journal_upload(&mut dispatcher, 3, "first journal notes marker");
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = Arc::new(IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![sample_journal(3)],
        ));

        start_remote_journal_upload_action(
            dispatcher.clone(),
            &spawner,
            client.clone(),
            IssueId::new(3),
            JournalId::new(1),
        );

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        let completion = recv_actions(&spawner, 1).remove(0);
        assert!(matches!(
            completion,
            Action::Journal(JournalAction::CompleteRemoteUpload { issue_id, journal_id, .. })
                if issue_id == IssueId::new(3) && journal_id == JournalId::new(1)
        ));
        assert_eq!(
            *client.uploaded_journal_notes.lock().unwrap(),
            vec!["edited notes".to_string()]
        );
    }

    #[test]
    fn start_remote_journal_upload_action_routes_the_failure_completion_to_worker_channel() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let mut dispatcher = Dispatcher::new();
        start_edited_journal_upload(&mut dispatcher, 3, "first journal notes marker");
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let mut client = IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![sample_journal(3)],
        );
        client.update_journal_result = Err(IssueUploadClient::network_error());
        let client = Arc::new(client);

        start_remote_journal_upload_action(
            dispatcher.clone(),
            &spawner,
            client.clone(),
            IssueId::new(3),
            JournalId::new(1),
        );

        let mut actions = recv_actions(&spawner, 2).into_iter();
        let notice = actions.next().expect("failure notice");
        assert!(matches!(notice, Action::Notice(NoticeAction::Push { .. })));
        let completion = actions.next().expect("upload failure");
        assert!(matches!(
            completion,
            Action::Journal(JournalAction::FailRemoteUpload { issue_id, journal_id, .. })
                if issue_id == IssueId::new(3) && journal_id == JournalId::new(1)
        ));
    }

    fn start_local_journal(dispatcher: &mut Dispatcher, issue_id: u16, notes: &str) {
        let issue = sample_issue_aggregate(
            issue_id,
            "subject",
            IssueStatusId::new(1),
            None,
            None,
            None,
            0,
        );
        dispatcher.dispatch(Action::Issue(IssueAction::Sync { issue }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::CreateLocal {
            issue_id: IssueId::new(issue_id),
        }));
        dispatcher.consume_action();
        dispatcher.dispatch(Action::Journal(JournalAction::EditLocalNotes {
            issue_id: IssueId::new(issue_id),
            notes: notes.to_string(),
        }));
        dispatcher.consume_action();
    }

    #[test]
    fn start_local_journal_upload_action_routes_the_upload_completion_to_worker_channel() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let mut dispatcher = Dispatcher::new();
        start_local_journal(&mut dispatcher, 3, "local notes");
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let client = Arc::new(IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![sample_journal(3)],
        ));

        start_local_journal_upload_action(
            dispatcher.clone(),
            &spawner,
            client.clone(),
            IssueId::new(3),
        );

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        let completion = recv_actions(&spawner, 1).remove(0);
        let Action::Journal(JournalAction::CompleteLocalUploadWithFetched { issue_id, journals }) =
            completion
        else {
            panic!("expected complete local upload action");
        };
        assert_eq!(issue_id, IssueId::new(3));
        assert_eq!(
            journals
                .iter()
                .map(|journal| journal.id)
                .collect::<Vec<_>>(),
            vec![JournalId::new(1)]
        );
        assert_eq!(
            *client.uploaded_issue_notes.lock().unwrap(),
            vec!["local notes".to_string()]
        );
    }

    #[test]
    fn start_local_journal_upload_action_routes_the_failure_completion_to_worker_channel() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let mut dispatcher = Dispatcher::new();
        start_local_journal(&mut dispatcher, 3, "local notes");
        let dispatcher = Rc::new(RefCell::new(dispatcher));
        let mut client = IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![sample_journal(3)],
        );
        client.update_issue_notes_result = Err(IssueUploadClient::network_error());
        let client = Arc::new(client);

        start_local_journal_upload_action(
            dispatcher.clone(),
            &spawner,
            client.clone(),
            IssueId::new(3),
        );

        let mut actions = recv_actions(&spawner, 2).into_iter();
        let notice = actions.next().expect("failure notice");
        assert!(matches!(notice, Action::Notice(NoticeAction::Push { .. })));
        let completion = actions.next().expect("upload failure");
        assert!(matches!(
            completion,
            Action::Journal(JournalAction::FailLocalUpload { issue_id, .. })
                if issue_id == IssueId::new(3)
        ));
    }

    fn journal_upload_app(dispatcher: Rc<RefCell<Dispatcher>>) -> AppComponent<'static> {
        let mut app = AppComponent::new(dispatcher.clone(), Some(IssueId::new(3)));
        app.update(
            dispatcher.clone(),
            dispatcher.borrow().store(),
            Rect::new(0, 0, 80, 24),
        );
        app
    }

    fn loaded_journal_upload_dispatcher() -> Dispatcher {
        let mut dispatcher = Dispatcher::new();
        crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher);
        dispatcher.dispatch(IssueAction::Load {
            id: IssueId::new(3),
        });
        while dispatcher.consume_actinos_len() > 0 {
            dispatcher.consume_action();
        }
        dispatcher
    }

    fn edited_remote_journal_dispatcher() -> Dispatcher {
        let mut dispatcher = loaded_journal_upload_dispatcher();
        dispatcher.dispatch(JournalAction::SyncFetched {
            issue_id: IssueId::new(3),
            journals: vec![
                parse_journal_yaml(JournalId::new(1)),
                parse_journal_yaml(JournalId::new(2)),
                parse_journal_yaml(JournalId::new(3)),
            ],
        });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditRemoteNotes {
            issue_id: IssueId::new(3),
            journal_id: JournalId::new(1),
            notes: "edited notes".to_string(),
        });
        dispatcher.consume_action();
        dispatcher
    }

    fn edited_issue_dispatcher() -> (Dispatcher, IssueAggregate) {
        let mut dispatcher = loaded_journal_upload_dispatcher();
        let server_issue = dispatcher.store().get_issue(IssueId::new(3)).0.clone();
        dispatcher.dispatch(IssueAction::UpdateDescription {
            id: IssueId::new(3),
            body: "locally edited description".to_string(),
        });
        dispatcher.consume_action();
        (dispatcher, server_issue)
    }

    fn local_journal_dispatcher() -> Dispatcher {
        let mut dispatcher = loaded_journal_upload_dispatcher();
        dispatcher.dispatch(JournalAction::CreateLocal {
            issue_id: IssueId::new(3),
        });
        dispatcher.consume_action();
        dispatcher.dispatch(JournalAction::EditLocalNotes {
            issue_id: IssueId::new(3),
            notes: "local notes".to_string(),
        });
        dispatcher.consume_action();
        dispatcher
    }

    fn move_focus_down(
        app: &mut AppComponent<'_>,
        dispatcher: Rc<RefCell<Dispatcher>>,
        count: usize,
    ) {
        for _ in 0..count {
            app.process_event(
                InputEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::none())),
                dispatcher.clone(),
            );
            app.update(
                dispatcher.clone(),
                dispatcher.borrow().store(),
                Rect::new(0, 0, 80, 24),
            );
        }
    }

    fn focus_remote_journal_notes(app: &mut AppComponent<'_>, dispatcher: Rc<RefCell<Dispatcher>>) {
        move_focus_down(app, dispatcher, 47);
    }

    fn focus_local_journal_notes(app: &mut AppComponent<'_>, dispatcher: Rc<RefCell<Dispatcher>>) {
        move_focus_down(app, dispatcher.clone(), 100);
        app.process_event(
            InputEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::none())),
            dispatcher.clone(),
        );
        app.update(
            dispatcher.clone(),
            dispatcher.borrow().store(),
            Rect::new(0, 0, 80, 24),
        );
    }

    fn press_ctrl_s(app: &mut AppComponent<'_>, dispatcher: Rc<RefCell<Dispatcher>>) {
        app.process_event(
            InputEvent::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::control())),
            dispatcher,
        );
    }

    fn route_worker_actions(
        spawner: &TokioBackgroundSpawner,
        expected: usize,
        dispatcher: Rc<RefCell<Dispatcher>>,
        app: &mut AppComponent<'_>,
    ) {
        let actions = recv_actions(spawner, expected);
        let acceptor = CompletionSpawner::new(actions);
        assert!(move_worker_action(&acceptor, dispatcher.clone()).is_none());
        update(dispatcher, app, Rect::new(0, 0, 80, 24));
    }

    fn assert_toast_contains(
        app: &AppComponent<'_>,
        dispatcher: Rc<RefCell<Dispatcher>>,
        expected: &str,
    ) {
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
        // 日本語はwide-character cell間に空白が入るため、本文はStoreで検証し、
        // render bufferでは単一幅のエラー原因だけをtoast表示の目印にする。
        assert!(
            dispatcher
                .borrow()
                .store()
                .get_notices()
                .iter()
                .any(|notice| notice.message.contains(expected))
        );
        assert!(rendered.contains("offline"), "rendered: {rendered}");
    }

    #[test]
    fn issue_upload_failure_routes_worker_actions_to_store_and_toast_and_retry_clears_failure() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let (initial_dispatcher, server_issue) = edited_issue_dispatcher();
        let dispatcher = Rc::new(RefCell::new(initial_dispatcher));
        let mut app = journal_upload_app(dispatcher.clone());
        let client = Arc::new(IssueUploadClient::failing_update(server_issue));

        press_ctrl_s(&mut app, dispatcher.clone());
        let Some(AppEffect::StartIssueUpload(id)) = app.take_effect() else {
            panic!("expected issue upload effect");
        };
        let future = start_issue_upload(dispatcher.clone(), client.clone(), id);
        spawner.spawn(future);
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        route_worker_actions(&spawner, 2, dispatcher.clone(), &mut app);

        let store = dispatcher.borrow();
        assert!(matches!(
            store.store().try_get_issue_state(id),
            Some(crate::stores::IssueState::Edited)
        ));
        assert_eq!(store.store().get_issue_property_diffs(id).len(), 1);
        assert_eq!(
            store.store().try_get_issue_upload_failure(id),
            Some("network error: offline")
        );
        assert_eq!(store.store().get_notices().len(), 1);
        drop(store);
        assert_toast_contains(&app, dispatcher.clone(), "Issue #3の保存に失敗しました");

        press_ctrl_s(&mut app, dispatcher.clone());
        let Some(AppEffect::StartIssueUpload(id)) = app.take_effect() else {
            panic!("expected retry issue upload effect");
        };
        let future = start_issue_upload(dispatcher.clone(), client, id);
        spawner.spawn(future);
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        assert_eq!(
            dispatcher.borrow().store().try_get_issue_upload_failure(id),
            None
        );
        assert_eq!(
            dispatcher
                .borrow()
                .store()
                .get_issue_property_diffs(id)
                .len(),
            1
        );
    }

    // Remote保存前GETの失敗がfocusを奪わないtoastになり、同じ入力位置から再保存できることを検証する。
    // FIXME: E2Eで良い粒度なので移行する
    #[test]
    fn remote_preflight_get_failure_shows_a_non_focusing_toast_and_retry_succeeds() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let initial_dispatcher = edited_remote_journal_dispatcher();
        let dispatcher = Rc::new(RefCell::new(initial_dispatcher));
        let mut app = journal_upload_app(dispatcher.clone());
        focus_remote_journal_notes(&mut app, dispatcher.clone());
        press_ctrl_s(&mut app, dispatcher.clone());
        let Some(AppEffect::StartRemoteJournalUpload {
            issue_id,
            journal_id,
        }) = app.take_effect()
        else {
            panic!("expected remote upload effect");
        };
        let client = Arc::new(IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![parse_journal_yaml(JournalId::new(1))],
        ));
        *client.get_failures_remaining.lock().unwrap() = 1;

        start_remote_journal_upload_action(
            dispatcher.clone(),
            &spawner,
            client.clone(),
            issue_id,
            journal_id,
        );
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        press_ctrl_s(&mut app, dispatcher.clone());
        // upload中の重複Ctrl+Sはeffectを生成せず、usecase呼び出し前に正常なno-opとなる。
        assert!(app.take_effect().is_none());
        route_worker_actions(&spawner, 2, dispatcher.clone(), &mut app);
        assert_toast_contains(
            &app,
            dispatcher.clone(),
            "Remote Journalの保存に失敗しました",
        );

        press_ctrl_s(&mut app, dispatcher.clone());
        let Some(AppEffect::StartRemoteJournalUpload {
            issue_id,
            journal_id,
        }) = app.take_effect()
        else {
            panic!("toast must not take focus from remote journal notes");
        };
        start_remote_journal_upload_action(
            dispatcher.clone(),
            &spawner,
            client.clone(),
            issue_id,
            journal_id,
        );
        update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
        route_worker_actions(&spawner, 1, dispatcher.clone(), &mut app);

        assert!(matches!(
            dispatcher
                .borrow()
                .store()
                .get_remote_journal(issue_id, journal_id)
                .state,
            crate::stores::RemoteJournalState::Synced
        ));
        assert_eq!(*client.get_requests.lock().unwrap(), 2);
        assert_eq!(client.uploaded_journal_notes.lock().unwrap().len(), 1);
    }

    // Remote PUTの失敗をtoastで通知した後もfocusを維持し、再保存でSyncedへ戻ることを検証する。
    #[test]
    fn remote_put_failure_shows_a_non_focusing_toast_and_retry_succeeds() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let initial_dispatcher = edited_remote_journal_dispatcher();
        let dispatcher = Rc::new(RefCell::new(initial_dispatcher));
        let mut app = journal_upload_app(dispatcher.clone());
        focus_remote_journal_notes(&mut app, dispatcher.clone());
        let client = Arc::new(IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![parse_journal_yaml(JournalId::new(1))],
        ));
        *client.journal_failures_remaining.lock().unwrap() = 1;

        for expected_actions in [2, 1] {
            press_ctrl_s(&mut app, dispatcher.clone());
            let Some(AppEffect::StartRemoteJournalUpload {
                issue_id,
                journal_id,
            }) = app.take_effect()
            else {
                panic!("toast must not take focus from remote journal notes");
            };
            start_remote_journal_upload_action(
                dispatcher.clone(),
                &spawner,
                client.clone(),
                issue_id,
                journal_id,
            );
            update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
            route_worker_actions(&spawner, expected_actions, dispatcher.clone(), &mut app);
            if expected_actions == 2 {
                assert_toast_contains(
                    &app,
                    dispatcher.clone(),
                    "Remote Journalの保存に失敗しました",
                );
            }
        }

        assert_eq!(*client.get_requests.lock().unwrap(), 2);
        assert_eq!(client.uploaded_journal_notes.lock().unwrap().len(), 2);
        assert!(matches!(
            dispatcher.borrow().store().get_remote_journal(3, 1).state,
            crate::stores::RemoteJournalState::Synced
        ));
    }

    // Local PUTの失敗をtoastで通知した後もfocusを維持し、再保存を完了できることを検証する。
    #[test]
    fn local_put_failure_shows_a_non_focusing_toast_and_retry_succeeds() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let initial_dispatcher = local_journal_dispatcher();
        let dispatcher = Rc::new(RefCell::new(initial_dispatcher));
        let mut app = journal_upload_app(dispatcher.clone());
        focus_local_journal_notes(&mut app, dispatcher.clone());
        let client = Arc::new(IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![sample_journal(3)],
        ));
        *client.local_failures_remaining.lock().unwrap() = 1;

        for expected_actions in [2, 1] {
            press_ctrl_s(&mut app, dispatcher.clone());
            let Some(AppEffect::StartLocalJournalUpload { issue_id }) = app.take_effect() else {
                panic!("toast must not take focus from local journal notes");
            };
            start_local_journal_upload_action(
                dispatcher.clone(),
                &spawner,
                client.clone(),
                issue_id,
            );
            update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
            route_worker_actions(&spawner, expected_actions, dispatcher.clone(), &mut app);
            if expected_actions == 2 {
                assert_toast_contains(
                    &app,
                    dispatcher.clone(),
                    "Local Journalの保存に失敗しました",
                );
            }
        }

        assert!(
            dispatcher
                .borrow()
                .store()
                .try_get_local_journal(3)
                .is_none()
        );
        assert_eq!(client.uploaded_issue_notes.lock().unwrap().len(), 2);
        assert_eq!(*client.get_requests.lock().unwrap(), 1);
    }

    // Local PUT成功後の確認GET失敗を部分成功として通知し、同じ入力位置から再保存できることを検証する。
    #[test]
    fn local_confirmation_get_failure_warns_about_possible_success_and_retry_succeeds() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let initial_dispatcher = local_journal_dispatcher();
        let dispatcher = Rc::new(RefCell::new(initial_dispatcher));
        let mut app = journal_upload_app(dispatcher.clone());
        focus_local_journal_notes(&mut app, dispatcher.clone());
        let client = Arc::new(IssueUploadClient::with_journals(
            sample_issue_aggregate(3, "subject", IssueStatusId::new(1), None, None, None, 0),
            vec![sample_journal(3)],
        ));
        *client.get_failures_remaining.lock().unwrap() = 1;

        for expected_actions in [2, 1] {
            press_ctrl_s(&mut app, dispatcher.clone());
            let Some(AppEffect::StartLocalJournalUpload { issue_id }) = app.take_effect() else {
                panic!("toast must not take focus from local journal notes");
            };
            start_local_journal_upload_action(
                dispatcher.clone(),
                &spawner,
                client.clone(),
                issue_id,
            );
            update(dispatcher.clone(), &mut app, Rect::new(0, 0, 80, 24));
            route_worker_actions(&spawner, expected_actions, dispatcher.clone(), &mut app);
            if expected_actions == 2 {
                assert_toast_contains(&app, dispatcher.clone(), "保存は完了した可能性がありますが");
            }
        }

        assert!(
            dispatcher
                .borrow()
                .store()
                .try_get_local_journal(3)
                .is_none()
        );
        assert_eq!(client.uploaded_issue_notes.lock().unwrap().len(), 2);
        assert_eq!(*client.get_requests.lock().unwrap(), 2);
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
        ) -> std::result::Result<FetchedIssue, RedmineClientError> {
            Err(self.unauthorized())
        }

        async fn update_issue(
            &self,
            _: &IssueAggregate,
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

        async fn update_issue_notes(
            &self,
            _: crate::vos::IssueId,
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
