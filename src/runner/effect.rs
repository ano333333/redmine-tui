//! `AppEffect`が要求する外部副作用をnative/Web共通runnerから起動する。

use std::{cell::RefCell, io, num::NonZeroUsize, rc::Rc, sync::Arc};

use crate::{
    clients::redmine::RedmineClient,
    components::{AppComponent, app::AppEffect},
    platform::{
        editor::{EditorOutcome, TextEditor},
        host::PlatformHost,
        runtime::{BackgroundSpawner, LocalTask},
    },
    stores::{Dispatcher, NoticeAction, NoticeId},
    usecases::redmine::{
        continue_remote_journal_upload, fetch_issue, fetch_project_issues_page, start_issue_upload,
        start_local_journal_upload, start_remote_journal_upload, upload_issue_action,
    },
    vos::{IssueId, JournalId, ProjectId},
};

pub(crate) type EditorSession<'a> = LocalTask<'a, io::Result<EditorOutcome>>;

pub(crate) fn handle_app_effect<'a, S, C, E, H>(
    effect: AppEffect,
    app_component: &mut AppComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    editor: &'a E,
    editor_session: &mut Option<EditorSession<'a>>,
    host: &mut H,
) where
    S: BackgroundSpawner,
    C: RedmineClient + Send + Sync + 'static,
    E: TextEditor,
    H: PlatformHost,
{
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

pub(crate) fn start_remote_journal_upload_action<S: BackgroundSpawner, C>(
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

pub(crate) fn start_local_journal_upload_action<S: BackgroundSpawner, C>(
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

pub(crate) fn continue_remote_journal_upload_action<S: BackgroundSpawner, C>(
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

pub(crate) fn start_project_issues_page_fetch<S: BackgroundSpawner, C>(
    dispatcher: Rc<RefCell<Dispatcher>>,
    spawner: &S,
    client: Arc<C>,
    project_id: ProjectId,
    page: NonZeroUsize,
) where
    C: RedmineClient + Send + Sync + 'static,
{
    let future = fetch_project_issues_page(dispatcher, client, project_id, page);
    spawner.spawn(async move { vec![future.await.into()] });
}

pub(crate) fn start_issue_fetch<S: BackgroundSpawner, C>(
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

    spawner.spawn(future);
}
