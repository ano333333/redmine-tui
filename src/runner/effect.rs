//! Redmine関連の`AppEffect`からbackground taskを起動するhelper。

use std::{cell::RefCell, num::NonZeroUsize, rc::Rc, sync::Arc};

use crate::{
    clients::redmine::RedmineClient,
    platform::runtime::BackgroundSpawner,
    stores::Dispatcher,
    usecases::redmine::{
        continue_remote_journal_upload, fetch_issue, fetch_project_issues_page,
        start_local_journal_upload, start_remote_journal_upload,
    },
    vos::{IssueId, JournalId, ProjectId},
};

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
