//! Componentが要求した外部副作用をnative/Web共通runnerから起動する。

use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, sync::Arc};

use ratatui::layout::Rect;

use crate::{
    clients::redmine::RedmineClient,
    components::{AppComponent, RequestSink},
    platform::{
        editor::{EditorOutcome, EditorRequest, TextEditor},
        host::{HostEvent, PlatformHost},
        runtime::{BackgroundSpawner, LocalTask},
    },
    stores::{Dispatcher, NoticeAction, NoticeId},
    usecases::{UsecaseOutput, UsecaseRequest, UsecaseTask, start_usecase},
};

use super::lifecycle::{handle_host_event, update};

pub(crate) type EditorSession<'a> = LocalTask<'a, io::Result<EditorOutcome>>;

/// Componentが`sink`に書いた要求を処理する。Usecaseの要求をすべて起動した後にeditorを起動する。
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_component_requests<'a, S, C, E, H>(
    sink: &mut RequestSink,
    requests: &mut VecDeque<UsecaseRequest>,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    editor: &'a E,
    editor_session: &mut Option<EditorSession<'a>>,
    host: &mut H,
) where
    S: BackgroundSpawner<Output = UsecaseOutput>,
    C: RedmineClient + Send + Sync + 'static,
    E: TextEditor,
    H: PlatformHost,
{
    requests.extend(sink.take_usecases());
    let area = host.area();
    drain_requests(
        requests,
        app_component,
        dispatcher.clone(),
        spawner,
        client,
        area,
    );
    if let Some(request) = sink.take_editor() {
        open_editor(
            request,
            app_component,
            dispatcher,
            editor,
            editor_session,
            host,
        );
    }
}

/// 入力をComponentへ渡し、発行された要求を起動してからComponentを更新する。
///
/// Usecaseが起動時にdispatchするActionを、この入力の後のupdateで反映済みにするため、
/// updateより先に要求を起動する。アプリを続ける場合にtrueを返す。
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_input<'a, S, C, E, H>(
    event: HostEvent,
    sink: &mut RequestSink,
    requests: &mut VecDeque<UsecaseRequest>,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    editor: &'a E,
    editor_session: &mut Option<EditorSession<'a>>,
    host: &mut H,
) -> bool
where
    S: BackgroundSpawner<Output = UsecaseOutput>,
    C: RedmineClient + Send + Sync + 'static,
    E: TextEditor,
    H: PlatformHost,
{
    let should_continue = handle_host_event(event, app_component, dispatcher.clone(), sink);
    handle_component_requests(
        sink,
        requests,
        app_component,
        dispatcher.clone(),
        spawner,
        client,
        editor,
        editor_session,
        host,
    );
    if should_continue {
        let area = host.area();
        update(dispatcher.clone(), app_component, area);
        app_component.update(dispatcher.clone(), dispatcher.borrow().store(), area);
    }
    should_continue
}

fn open_editor<'a, E, H>(
    request: EditorRequest,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    editor: &'a E,
    editor_session: &mut Option<EditorSession<'a>>,
    host: &mut H,
) where
    E: TextEditor,
    H: PlatformHost,
{
    // FIXME: 実terminalとexternal editor processを使い、editorの成否にかかわらず長時間滞在後もNoticeが残ることをE2E testで確認する。
    // FIXME: 実terminalとexternal editor processを使い、editor失敗時のnotice追加とfocus/cursor維持をE2E testで確認する。
    // FIXME: 実terminalとexternal editor processを使い、アプリ終了時にterminal状態が復元されeditor processがkillされることをE2E testで確認する。
    if let Err(error) = host.suspend_for_editor() {
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

pub(crate) fn handle_editor_failure(
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

pub(crate) fn editor_failure_notice_action(error: &dyn std::fmt::Display) -> NoticeAction {
    NoticeAction::Push {
        id: NoticeId::new(),
        message: format!("エディタによる編集に失敗しました: {error}"),
    }
}

/// `requests`を先頭から1件ずつ起動し、起動するたびにupdateする。
///
/// 次の要求を起動する時点で前の要求の起動Actionを消費済みにするため、同じIssueへの
/// `FetchIssue`が続いても、2件目は1件目の取得開始を観測して起動しない。
pub(crate) fn drain_requests<S, C>(
    requests: &mut VecDeque<UsecaseRequest>,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    area: Rect,
) where
    S: BackgroundSpawner<Output = UsecaseOutput>,
    C: RedmineClient + Send + Sync + 'static,
{
    while let Some(request) = requests.pop_front() {
        spawn_usecase(
            spawner,
            start_usecase(request, dispatcher.clone(), client.clone()),
        );
        update(dispatcher.clone(), app_component, area);
    }
}

fn spawn_usecase<S>(spawner: &S, task: Option<UsecaseTask>)
where
    S: BackgroundSpawner<Output = UsecaseOutput>,
{
    if let Some(task) = task {
        spawner.spawn(task);
    }
}
