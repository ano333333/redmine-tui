use std::cell::RefCell;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use crate::components::issue::{
    EventProcessResult as IssueEventProcessResult, IssueComponent, IssueDetailEventProcessResult,
};
use crate::components::issue_property_conflict_popup::{
    EventProcessResult as IssuePropertyConflictEventProcessResult, IssuePropertyConflictComponent,
};
use crate::components::issue_select_popup::component::EventProcessResult as IssueSelectPopupEventProcessResult;
use crate::components::issue_select_popup::component::{
    Effect as IssueSelectPopupEffect, IssueSelectPopupComponent,
};
use crate::stores::{Action, Dispatcher, IssueAction, JournalAction, Store};
use crate::usecases::issue_popup_options::{
    assigned_to_popup_observer, build_assigned_to_options, build_category_options,
    build_done_ratio_options, build_issue_status_options, build_target_version_options,
    category_popup_observer, current_due_date, current_start_date, done_ratio_popup_observer,
    due_date_popup_observer, issue_status_popup_observer, start_date_popup_observer,
    target_version_popup_observer,
};
use crate::usecases::redmine::{cancel_issue_upload, continue_issue_upload};
use crate::vos::{
    EntityIdValue, IssueId, IssuePropertyDiff, JournalId, ProjectId, TimeEntityActivityId,
};
use crate::widgets::ToastWidget;

use super::date_picker_popup::component::{
    DatePickerPopupComponent, EventProcessResult as DatePickerPopupEventProcessResult,
};
use super::select_box_popup::{
    EventProcessResult as SelectBoxPopupEventProcessResult, SelectBoxPopupComponent,
};
use super::spent_time_input_popup::{
    EventProcessResult as SpentTimeInputPopupEventProcessResult, SpentTimeInputPopupComponent,
};

pub struct EditorRequest {
    pub initial_text: String,
}

pub struct EditorResponse {
    pub edited_text: String,
}

pub enum AppEffect {
    FetchIssue(IssueId),
    FetchProjectIssuesPage {
        project_id: ProjectId,
        page: NonZeroUsize,
    },
    OpenEditor(EditorRequest),
    StartIssueUpload(IssueId),
    ContinueIssueUpload {
        id: IssueId,
        diffs: Vec<IssuePropertyDiff>,
    },
}

enum PendingEditorContext {
    IssueBody {
        id: IssueId,
    },
    Journal {
        issue_id: IssueId,
        journal_id: JournalId,
    },
}

enum PopupComponent<'a> {
    SelectBox(SelectBoxPopupComponent<'a>),
    SpentTimeInput(SpentTimeInputPopupComponent<'a>),
    DatePicker(DatePickerPopupComponent<'a>),
    IssueSelect(IssueSelectPopupComponent),
    IssuePropertyConflict {
        issue_id: IssueId,
        component: IssuePropertyConflictComponent,
    },
}

pub struct AppComponent<'a> {
    issue_component: Option<IssueComponent>,
    // popup追加の際は末尾に追加する、先頭要素が最奥に表示される
    popup_components: VecDeque<Rc<RefCell<PopupComponent<'a>>>>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pending_effect: Option<AppEffect>,
    pending_editor_context: Option<PendingEditorContext>,
}

impl<'a> AppComponent<'a> {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>, issue_id: Option<IssueId>) -> Self {
        let (issue_component, issue_result) = match issue_id {
            Some(issue_id) => {
                let dispatcher_ref = dispatcher.borrow();
                let (component, result) = IssueComponent::new(dispatcher_ref.store(), issue_id);
                (Some(component), result)
            }
            None => (None, None),
        };
        let mut app = AppComponent {
            issue_component,
            popup_components: VecDeque::new(),
            dispatcher,
            pending_effect: None,
            pending_editor_context: None,
        };
        if let Some(result) = issue_result {
            app.handle_issue_component_result(result);
        }
        if issue_id.is_none() {
            app.open_issue_select_popup(None);
        }
        app
    }

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
    pub fn process_event(&mut self, event: Event, dispatcher: Rc<RefCell<Dispatcher>>) {
        if self.popup_components.back().is_some() {
            self.process_popup_event(event, dispatcher);
        } else if self.issue_component.is_some() {
            self.process_issue_event(event, dispatcher);
        }
    }

    /// 最前面のpopupへイベントを渡し、結果に応じてpopup stackを更新する。
    fn process_popup_event(&mut self, event: Event, dispatcher: Rc<RefCell<Dispatcher>>) {
        let popup_component_rc = self
            .popup_components
            .back()
            .expect("caller checked that a popup exists")
            .clone();
        match &mut *(popup_component_rc.borrow_mut()) {
            PopupComponent::SelectBox(popup_component) => {
                let result = popup_component.process_event(event, self.dispatcher.clone());
                match result {
                    Some(SelectBoxPopupEventProcessResult::Entered)
                    | Some(SelectBoxPopupEventProcessResult::Quited) => {
                        self.popup_components.pop_back();
                    }
                    None => {}
                }
            }
            PopupComponent::SpentTimeInput(popup_component) => {
                let result = popup_component.process_event(event);
                match result {
                    Some(SpentTimeInputPopupEventProcessResult::Quited) => {
                        self.popup_components.pop_back();
                    }
                    Some(SpentTimeInputPopupEventProcessResult::OpenTimeEntityActivitiesPopup) => {
                        let popup_component = Self::create_spent_time_input_popup_component(
                            dispatcher.clone(),
                            popup_component_rc.clone(),
                        );
                        self.popup_components.push_back(popup_component);
                    }
                    Some(SpentTimeInputPopupEventProcessResult::Submited) => {
                        // FIXME: Storeの更新
                        self.popup_components.pop_back();
                    }
                    None => {}
                }
            }
            PopupComponent::DatePicker(popup_component) => {
                let result = popup_component.process_event(event);
                match result {
                    Some(DatePickerPopupEventProcessResult::Entered)
                    | Some(DatePickerPopupEventProcessResult::Canceled) => {
                        self.popup_components.pop_back();
                    }
                    None => {}
                }
            }
            PopupComponent::IssueSelect(popup_component) => {
                let result = {
                    let dispatcher = dispatcher.borrow();
                    popup_component.process_event(event, dispatcher.store())
                };
                let effect = popup_component.take_effect();
                if let Some(effect) = effect {
                    self.install_issue_select_popup_effect(effect);
                }
                match result {
                    Some(IssueSelectPopupEventProcessResult::Selected { issue_id }) => {
                        self.select_issue(issue_id);
                    }
                    Some(IssueSelectPopupEventProcessResult::Quited) => {
                        self.close_issue_select_popup();
                    }
                    None => {}
                }
            }
            PopupComponent::IssuePropertyConflict {
                issue_id,
                component,
            } => match component.process_event(event) {
                Some(IssuePropertyConflictEventProcessResult::Canceled) => {
                    cancel_issue_upload(&mut dispatcher.borrow_mut(), *issue_id);
                    self.popup_components.pop_back();
                }
                Some(IssuePropertyConflictEventProcessResult::Continued { diffs }) => {
                    let retry_diffs =
                        continue_issue_upload(&mut dispatcher.borrow_mut(), *issue_id, diffs);
                    self.pending_effect = Some(AppEffect::ContinueIssueUpload {
                        id: *issue_id,
                        diffs: retry_diffs,
                    });
                    self.popup_components.pop_back();
                }
                None => {}
            },
        }
    }

    /// IssueComponentへイベントを渡し、結果に応じてpopupの開閉やeffectの設置を行う。
    fn process_issue_event(&mut self, event: Event, dispatcher: Rc<RefCell<Dispatcher>>) {
        let (result, issue_id) = {
            let issue_component = self
                .issue_component
                .as_mut()
                .expect("caller checked that IssueComponent exists");
            (
                issue_component.process_event(event, self.dispatcher.clone()),
                issue_component.issue_id(),
            )
        };
        match result {
            Some(IssueEventProcessResult::FetchRequested { id }) => {
                self.install_effect(AppEffect::FetchIssue(id));
            }
            Some(IssueEventProcessResult::OpenIssueSelectPopup) => {
                self.open_issue_select_popup(Some(issue_id));
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::EditIssueBodyRequested { id, body },
            )) => {
                self.pending_editor_context = Some(PendingEditorContext::IssueBody { id });
                self.pending_effect =
                    Some(AppEffect::OpenEditor(EditorRequest { initial_text: body }));
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenIssueStatusPopup,
            )) => {
                let (items, focused_index) =
                    build_issue_status_options(dispatcher.borrow().store());
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    issue_status_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenAssignedToPopup,
            )) => {
                let (items, focused_index) =
                    build_assigned_to_options(dispatcher.borrow().store(), issue_id);
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    true,
                    assigned_to_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenTargetVersionPopup,
            )) => {
                let (items, focused_index) =
                    build_target_version_options(dispatcher.borrow().store(), issue_id);
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    true,
                    target_version_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenStartDatePopup,
            )) => {
                let selected_date = current_start_date(dispatcher.borrow().store(), issue_id);
                self.popup_components
                    .push_back(Rc::new(RefCell::new(PopupComponent::DatePicker(
                        DatePickerPopupComponent::new(
                            selected_date,
                            start_date_popup_observer(dispatcher.clone(), issue_id),
                        ),
                    ))));
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenDueDatePopup,
            )) => {
                let selected_date = current_due_date(dispatcher.borrow().store(), issue_id);
                self.popup_components
                    .push_back(Rc::new(RefCell::new(PopupComponent::DatePicker(
                        DatePickerPopupComponent::new(
                            selected_date,
                            due_date_popup_observer(dispatcher.clone(), issue_id),
                        ),
                    ))));
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenDoneRatioPopup,
            )) => {
                let (items, focused_index) =
                    build_done_ratio_options(dispatcher.borrow().store(), issue_id);
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    done_ratio_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenCategoryPopup,
            )) => {
                let (items, focused_index) =
                    build_category_options(dispatcher.borrow().store(), issue_id);
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    true,
                    category_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenSpentTimeInputPopup,
            )) => {
                self.popup_components.push_back(Rc::new(RefCell::new(
                    PopupComponent::SpentTimeInput(SpentTimeInputPopupComponent::new(
                        dispatcher.borrow().store(),
                    )),
                )));
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::StartIssueUpload,
            )) => {
                self.pending_effect = Some(AppEffect::StartIssueUpload(issue_id));
            }
            Some(IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::EditJournalRequested {
                    issue_id,
                    id,
                    notes,
                },
            )) => {
                self.pending_editor_context = Some(PendingEditorContext::Journal {
                    issue_id,
                    journal_id: id,
                });
                self.pending_effect = Some(AppEffect::OpenEditor(EditorRequest {
                    initial_text: notes,
                }));
            }
            None => {}
        }
    }

    fn select_issue(&mut self, issue_id: IssueId) {
        assert!(
            self.pending_effect.is_none(),
            "AppComponent already has a pending effect when selecting an issue"
        );

        self.popup_components.pop_back();
        let (issue_component, result) = {
            let dispatcher = self.dispatcher.borrow();
            IssueComponent::new(dispatcher.store(), issue_id)
        };
        self.issue_component = Some(issue_component);
        if let Some(result) = result {
            self.handle_issue_component_result(result);
        }
    }

    fn handle_issue_component_result(&mut self, result: IssueEventProcessResult) {
        match result {
            IssueEventProcessResult::FetchRequested { id } => {
                self.install_effect(AppEffect::FetchIssue(id));
            }
            IssueEventProcessResult::OpenIssueSelectPopup | IssueEventProcessResult::Detail(_) => {
                panic!("IssueComponent::new returned an event-only result")
            }
        }
    }

    fn install_effect(&mut self, effect: AppEffect) {
        assert!(
            self.pending_effect.is_none(),
            "AppComponent already has a pending effect"
        );
        self.pending_effect = Some(effect);
    }

    fn install_issue_select_popup_effect(&mut self, effect: IssueSelectPopupEffect) {
        match effect {
            IssueSelectPopupEffect::FetchProjectIssuesPage { project_id, page } => {
                self.install_effect(AppEffect::FetchProjectIssuesPage { project_id, page });
            }
        }
    }

    fn open_issue_select_popup(&mut self, focused_issue_id: Option<IssueId>) {
        let mut popup = {
            let dispatcher = self.dispatcher.borrow();
            IssueSelectPopupComponent::new(dispatcher.store(), focused_issue_id)
        };
        if let Some(effect) = popup.take_effect() {
            self.install_issue_select_popup_effect(effect);
        }
        self.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(popup))));
    }

    fn close_issue_select_popup(&mut self) {
        self.popup_components.pop_back();
    }

    /// SelectBoxPopupComponentを構築し、popup stackへ積む。
    fn push_select_box_popup(
        &mut self,
        items: &[(u16, String)],
        focused_index: usize,
        include_none: bool,
        observer: Box<dyn FnMut(Option<u16>) + 'a>,
    ) {
        self.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::SelectBox(
                SelectBoxPopupComponent::new(items, focused_index, include_none, observer),
            ))));
    }

    /// popupへ先にキーを渡し、未処理のqだけをアプリ終了として返す。
    pub fn handle_key_event(&mut self, event: Event, dispatcher: Rc<RefCell<Dispatcher>>) -> bool {
        // FIXME: handle_key_eventが「内部で処理したが他のコンポーネントに影響がない」値を返すようにする
        let is_q = matches!(
            event,
            Event::Key(key) if key.code == crossterm::event::KeyCode::Char('q')
        );
        let popup_before = self.popup_components.back().cloned();
        self.process_event(event, dispatcher);
        if !is_q {
            return true;
        }
        match (popup_before, self.popup_components.back()) {
            (Some(before), Some(after)) => !Rc::ptr_eq(&before, after),
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store, area: Rect) {
        if let Some(issue_component) = &self.issue_component
            && let Some((server_issue, conflicts)) =
                store.get_issue_upload_conflict(issue_component.issue_id())
            && !self.popup_components.iter().any(|popup| {
                matches!(
                    &*popup.borrow(),
                    PopupComponent::IssuePropertyConflict { issue_id, .. }
                        if *issue_id == issue_component.issue_id()
                )
            })
        {
            self.popup_components.push_back(Rc::new(RefCell::new(
                PopupComponent::IssuePropertyConflict {
                    issue_id: issue_component.issue_id(),
                    component: IssuePropertyConflictComponent::new(
                        server_issue.clone(),
                        conflicts.to_vec(),
                    ),
                },
            )));
        }

        if let Some(issue_component) = &mut self.issue_component {
            issue_component.update(dispatcher, store, (area.width, area.height));
        }

        for popup_component in &self.popup_components {
            if let PopupComponent::IssueSelect(popup_component) = &mut *popup_component.borrow_mut()
            {
                popup_component.update(store, area);
            }
            if let PopupComponent::IssuePropertyConflict { component, .. } =
                &mut *popup_component.borrow_mut()
            {
                component.update(area);
            }
        }
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, area: Rect) {
        if let Some(issue_component) = &self.issue_component {
            issue_component.render(store, frame, area);
        }
        self.render_popup_component(frame, area, store);
        // popupより後に描画して最前面へ重ねる。toastはfocusを持たずcursor位置も変えない。
        frame.render_widget(create_toast_widget(store), area);
        if let Some(cursor_position) = self.cursor_position(store, area) {
            frame.set_cursor_position(cursor_position);
        }
    }

    pub fn take_effect(&mut self) -> Option<AppEffect> {
        self.pending_effect.take()
    }

    pub fn handle_editor_response(&mut self, response: EditorResponse) {
        match self.pending_editor_context.take() {
            Some(PendingEditorContext::IssueBody { id }) => {
                self.dispatcher
                    .borrow_mut()
                    .dispatch(IssueAction::UpdateDescription {
                        id,
                        body: response.edited_text,
                    });
            }
            Some(PendingEditorContext::Journal {
                issue_id,
                journal_id,
            }) => {
                self.dispatcher.borrow_mut().dispatch(Action::Journal(
                    JournalAction::EditRemoteNotes {
                        issue_id,
                        journal_id,
                        notes: response.edited_text,
                    },
                ));
            }
            None => {}
        }
    }

    fn render_popup_component(&self, frame: &mut Frame, area: Rect, store: &Store) {
        for popup_component in &self.popup_components {
            let popup_component = popup_component.clone();
            match &mut *(popup_component.borrow_mut()) {
                PopupComponent::SelectBox(popup_component) => {
                    let widget = popup_component.create_widget();
                    frame.render_widget(widget, area);
                }
                PopupComponent::SpentTimeInput(popup_component) => {
                    let widget = popup_component.create_widget(store);
                    frame.render_widget(widget, area);
                }
                PopupComponent::DatePicker(popup_component) => {
                    let widget = popup_component.create_widget();
                    frame.render_widget(widget, area);
                }
                PopupComponent::IssueSelect(popup_component) => {
                    let widget = popup_component.create_widget(store);
                    frame.render_widget(widget, area);
                }
                PopupComponent::IssuePropertyConflict { component, .. } => {
                    let widget = component.create_widget(area);
                    frame.render_widget(widget, area);
                }
            }
        }
    }

    fn cursor_position(&self, store: &Store, area: Rect) -> Option<Position> {
        if let Some(popup_component) = self.popup_components.back() {
            let popup_component = popup_component.borrow();
            match &*popup_component {
                PopupComponent::SelectBox(_) => None,
                PopupComponent::SpentTimeInput(popup_component) => {
                    popup_component.cursor_position(area)
                }
                PopupComponent::DatePicker(_) => None,
                PopupComponent::IssueSelect(_) => None,
                PopupComponent::IssuePropertyConflict { component, .. } => {
                    component.cursor_position(area)
                }
            }
        } else if let Some(issue_component) = &self.issue_component {
            issue_component.calc_cursor_position(store, area)
        } else {
            None
        }
    }

    fn create_spent_time_input_popup_component(
        dispatcher: Rc<RefCell<Dispatcher>>,
        popup_component: Rc<RefCell<PopupComponent<'a>>>,
    ) -> Rc<RefCell<PopupComponent<'a>>> {
        let dispatcher = dispatcher.borrow();
        let acts = dispatcher.store().get_time_entity_activities();
        let act_names = acts
            .iter()
            .map(|(id, status)| (id.get(), status.name.clone()))
            .collect::<Vec<_>>();
        let focused_index = acts
            .iter()
            .enumerate()
            .find(|(_, (_, act))| act.is_default)
            .map_or(0, |(id, _)| id);
        drop(dispatcher);
        let popup_component_weak = Rc::downgrade(&popup_component);
        Rc::new(RefCell::new(PopupComponent::SelectBox(
            SelectBoxPopupComponent::new(
                &act_names,
                focused_index,
                false,
                Box::new(move |act_id| {
                    if let (Some(act_id), Some(popup_component)) =
                        (act_id, popup_component_weak.upgrade())
                    {
                        let popup_component = &mut *popup_component.borrow_mut();
                        if let PopupComponent::SpentTimeInput(popup_component) = popup_component {
                            popup_component.on_time_entity_activity_selected(
                                TimeEntityActivityId::new(act_id),
                            );
                        }
                    }
                }),
            ),
        )))
    }
}

fn create_toast_widget(store: &Store) -> ToastWidget {
    ToastWidget::new(
        store
            .get_notices()
            .iter()
            .map(|notice| notice.message.clone())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::entities::{Issue, ProjectIssuesPage};
    use crate::libs::yaml::parse_journal_yaml;
    use crate::stores::{
        JournalAction, NoticeAction, NoticeId, ProjectIssuesAction, RemoteJournalState,
    };
    use crate::vos::IssuePropertyDiff;
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    #[test]
    fn toast_widget_reflects_store_notices() {
        let mut store = Store::new();
        store.consume_action(Action::Notice(NoticeAction::Push {
            id: NoticeId::new(),
            message: "Issueの保存に失敗しました: 接続が切れました".to_string(),
            created_at: crate::test_support::local_datetime("2026-02-16T10:00:00+09:00"),
        }));

        let widget = create_toast_widget(&store);

        assert_eq!(
            widget.messages,
            vec!["Issueの保存に失敗しました: 接続が切れました".to_string()]
        );
    }

    #[test]
    fn toast_widget_is_empty_when_store_has_no_notices() {
        let store = Store::new();

        let widget = create_toast_widget(&store);

        assert!(widget.messages.is_empty());
    }

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn loaded_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher_ref);
            dispatcher_ref.dispatch(IssueAction::Load { id: 3.into() });
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
        }
        dispatcher
    }

    fn mark_issue_edited(dispatcher: Rc<RefCell<Dispatcher>>, id: IssueId) {
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::UpdateDescription {
                id,
                body: "updated body".to_string(),
            });
        dispatcher.borrow_mut().consume_action();
    }

    fn loaded_dispatcher_with_edited_issue() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = loaded_dispatcher();
        mark_issue_edited(dispatcher.clone(), IssueId::new(3));
        dispatcher
    }

    fn app_with_issue_property_conflict_popup() -> (Rc<RefCell<Dispatcher>>, AppComponent<'static>)
    {
        let dispatcher = loaded_dispatcher_with_edited_issue();
        let conflicts = dispatcher
            .borrow()
            .store()
            .get_issue_property_diffs(IssueId::new(3))
            .to_vec();
        let mut server_issue = dispatcher.borrow().store().get_issue(3).unwrap().0.clone();
        server_issue.issue.description = "server body".to_string();
        {
            let mut dispatcher = dispatcher.borrow_mut();
            dispatcher.dispatch(IssueAction::StartUpload { id: 3.into() });
            dispatcher.dispatch(IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
            });
            dispatcher.consume_action();
            dispatcher.consume_action();
        }
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        (dispatcher, app)
    }

    fn dispatcher_with_issue() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::Load { id: 3.into() });
        dispatcher.borrow_mut().consume_action();
        dispatcher
    }

    fn dispatcher_with_edited_selectable_issues() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = dispatcher_with_selectable_issues();
        mark_issue_edited(dispatcher.clone(), IssueId::new(3));
        dispatcher
    }

    fn dispatcher_with_selectable_issues() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher_ref);
            dispatcher_ref.dispatch(IssueAction::Load { id: 1.into() });
            dispatcher_ref.dispatch(IssueAction::Load { id: 3.into() });
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
            let request_id = crate::stores::ProjectIssuesRequestId::new();
            dispatcher_ref.dispatch(ProjectIssuesAction::StartLoading {
                request_id,
                project_id: 1.into(),
                page: NonZeroUsize::MIN,
            });
            dispatcher_ref.dispatch(ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id: 1.into(),
                page: NonZeroUsize::MIN,
                result: selectable_project_issues_page(),
            });
            dispatcher_ref.consume_action();
            dispatcher_ref.consume_action();
        }
        dispatcher
    }

    fn selectable_project_issues_page() -> ProjectIssuesPage {
        ProjectIssuesPage {
            issues: [1_u16, 3_u16, 42_u16]
                .into_iter()
                .map(|id| Issue {
                    id: id.into(),
                    project_id: 1.into(),
                    subject: format!("issue{id}"),
                    description: "body".to_string(),
                    status_id: 1.into(),
                })
                .collect(),
            total_count: 3,
            offset: 0,
            limit: 50,
        }
    }

    fn complete_initial_popup_page_fetch(
        app: &mut AppComponent<'_>,
        dispatcher: Rc<RefCell<Dispatcher>>,
    ) {
        let Some(AppEffect::FetchProjectIssuesPage { project_id, page }) = app.take_effect() else {
            panic!("expected the popup's initial project page effect")
        };
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        let request_id = crate::stores::ProjectIssuesRequestId::new();
        dispatcher
            .borrow_mut()
            .dispatch(ProjectIssuesAction::StartLoading {
                request_id,
                project_id,
                page,
            });
        dispatcher.borrow_mut().consume_action();
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        dispatcher
            .borrow_mut()
            .dispatch(ProjectIssuesAction::LoadSucceeded {
                request_id,
                project_id,
                page,
                result: selectable_project_issues_page(),
            });
        dispatcher.borrow_mut().consume_action();
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
    }

    fn focus_property_line(
        app: &mut AppComponent<'_>,
        dispatcher: Rc<RefCell<Dispatcher>>,
        line: u16,
    ) {
        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        for _ in 0..line {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }
    }

    fn select_next_day_in_open_date_picker(
        app: &mut AppComponent<'_>,
        dispatcher: Rc<RefCell<Dispatcher>>,
    ) {
        for _ in 0..3 {
            app.process_event(key_event(KeyCode::Tab), dispatcher.clone());
        }
        app.process_event(key_event(KeyCode::Char('l')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher);
    }

    fn loaded_dispatcher_with_journals() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = loaded_dispatcher();
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            dispatcher_ref.dispatch(Action::Journal(JournalAction::SyncFetched {
                issue_id: IssueId::new(3),
                journals: vec![
                    parse_journal_yaml(JournalId::new(1)),
                    parse_journal_yaml(JournalId::new(2)),
                    parse_journal_yaml(JournalId::new(3)),
                ],
            }));
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
        }
        dispatcher
    }

    /// Property(15行) -> Body -> ChildrenList -> JournalsList(先頭Journalのdetail)
    /// の順にフォーカスを送り、JournalsList内の1件目JournalのNotes位置に到達させる
    fn focus_first_journal_notes(app: &mut AppComponent<'_>, dispatcher: Rc<RefCell<Dispatcher>>) {
        for _ in 0..54 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
            app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        }
    }

    #[test]
    fn new_with_loaded_initial_issue_creates_issue_component_without_fetch_effect() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher, Some(3.into()));

        assert_eq!(
            app.issue_component.as_ref().unwrap().issue_id(),
            IssueId::new(3)
        );
        assert!(app.take_effect().is_none());
    }

    #[test]
    fn new_with_unknown_initial_issue_requests_exactly_one_fetch() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut app = AppComponent::new(dispatcher, Some(42.into()));

        assert_eq!(
            app.issue_component.as_ref().unwrap().issue_id(),
            IssueId::new(42)
        );
        assert!(matches!(
            app.take_effect(),
            Some(AppEffect::FetchIssue(id)) if id == IssueId::new(42)
        ));
        assert!(app.take_effect().is_none());
    }

    #[test]
    fn new_with_failed_initial_issue_requests_exactly_one_fetch() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartFetching { id: 42.into() });
        dispatcher.borrow_mut().consume_action();
        dispatcher.borrow_mut().dispatch(IssueAction::FetchFailed {
            id: 42.into(),
            message: "offline".to_string(),
        });
        dispatcher.borrow_mut().consume_action();
        let mut app = AppComponent::new(dispatcher, Some(42.into()));

        assert!(matches!(app.take_effect(), Some(AppEffect::FetchIssue(id)) if id == 42));
        assert!(app.take_effect().is_none());
    }

    #[test]
    fn r_requests_retry_after_mounted_issue_transitions_to_fetch_failed() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartFetching { id: 42.into() });
        dispatcher.borrow_mut().consume_action();
        let mut app = AppComponent::new(dispatcher.clone(), Some(42.into()));
        assert!(app.take_effect().is_none());
        dispatcher.borrow_mut().dispatch(IssueAction::FetchFailed {
            id: 42.into(),
            message: "offline".to_string(),
        });
        dispatcher.borrow_mut().consume_action();
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        app.process_event(key_event(KeyCode::Char('r')), dispatcher);

        assert!(matches!(app.take_effect(), Some(AppEffect::FetchIssue(id)) if id == 42));
        assert!(app.take_effect().is_none());
    }

    #[test]
    #[should_panic(expected = "AppComponent already has a pending effect")]
    fn a_second_fetch_request_cannot_replace_a_pending_effect() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::StartFetching { id: 42.into() });
        dispatcher.borrow_mut().consume_action();
        dispatcher.borrow_mut().dispatch(IssueAction::FetchFailed {
            id: 42.into(),
            message: "offline".to_string(),
        });
        dispatcher.borrow_mut().consume_action();
        let mut app = AppComponent::new(dispatcher.clone(), Some(42.into()));

        app.process_event(key_event(KeyCode::Char('r')), dispatcher);
    }

    #[test]
    fn new_without_initial_issue_opens_issue_select_popup() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone(), None);

        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        assert!(app.issue_component.is_none());
        assert_eq!(app.popup_components.len(), 1);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::IssueSelect(_)
        ));
        assert_eq!(app.cursor_position(dispatcher.borrow().store(), AREA), None);
        crate::test_support::render_frame_snapshot(
            "app_with_initial_issue_select_popup",
            AREA.width,
            AREA.height,
            |frame| app.render(dispatcher.borrow().store(), frame, AREA),
        );
    }

    #[test]
    fn q_key_on_initial_issue_select_popup_returns_to_empty_main_screen() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone(), None);

        app.process_event(key_event(KeyCode::Char('q')), dispatcher);

        assert!(app.issue_component.is_none());
        assert!(app.popup_components.is_empty());
    }

    #[test]
    fn process_event_without_initial_issue_ignores_issue_detail_keys() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone(), None);
        let _ = app.take_effect();
        app.process_event(key_event(KeyCode::Char('q')), dispatcher.clone());

        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('e'),
            KeyCode::Char('u'),
        ] {
            app.process_event(key_event(code), dispatcher.clone());
        }

        assert!(app.issue_component.is_none());
        assert!(app.popup_components.is_empty());
        assert!(app.take_effect().is_none());
    }

    #[test]
    fn update_opens_issue_property_conflict_popup_only_once() {
        let dispatcher = loaded_dispatcher_with_edited_issue();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        let conflicts = dispatcher
            .borrow()
            .store()
            .get_issue_property_diffs(IssueId::new(3))
            .to_vec();
        let server_issue = dispatcher.borrow().store().get_issue(3).unwrap().0.clone();
        {
            let mut dispatcher = dispatcher.borrow_mut();
            dispatcher.dispatch(IssueAction::StartUpload { id: 3.into() });
            dispatcher.dispatch(IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
            });
            dispatcher.consume_action();
            dispatcher.consume_action();
        }

        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        assert_eq!(app.popup_components.len(), 1);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::IssuePropertyConflict { issue_id, .. }
                if *issue_id == IssueId::new(3)
        ));
    }

    #[test]
    fn q_key_on_issue_property_conflict_popup_requests_upload_cancellation() {
        let (dispatcher, mut app) = app_with_issue_property_conflict_popup();

        app.process_event(key_event(KeyCode::Char('q')), dispatcher.clone());

        assert!(app.popup_components.is_empty());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 2);
    }

    #[test]
    fn cancel_button_on_issue_property_conflict_popup_requests_upload_cancellation() {
        let (dispatcher, mut app) = app_with_issue_property_conflict_popup();

        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('h')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(app.popup_components.is_empty());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 2);
    }

    #[test]
    fn continue_button_on_issue_property_conflict_popup_requests_upload_retry() {
        let (dispatcher, mut app) = app_with_issue_property_conflict_popup();

        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(app.popup_components.is_empty());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);
        let Some(AppEffect::ContinueIssueUpload { id, diffs }) = app.take_effect() else {
            panic!("続行時はIssueアップロード再試行effectが必要です");
        };
        assert_eq!(id, IssueId::new(3));
        assert_eq!(
            diffs,
            vec![IssuePropertyDiff::Description(IssueDescriptionDiff {
                before: "server body".to_string(),
                after: "updated body".to_string(),
            })]
        );

        dispatcher.borrow_mut().consume_action();
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        assert!(app.popup_components.is_empty());
        assert!(
            dispatcher
                .borrow()
                .store()
                .get_issue_upload_conflict(IssueId::new(3))
                .is_none()
        );
    }

    #[test]
    fn enter_on_issue_select_popup_creates_issue_detail_component_when_no_issue_is_displayed() {
        let dispatcher = dispatcher_with_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), None);
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone());

        app.process_event(key_event(KeyCode::Enter), dispatcher);

        assert!(app.popup_components.is_empty());
        assert_eq!(app.issue_component.unwrap().issue_id(), IssueId::new(1));
    }

    #[test]
    fn category_popup_includes_none_and_can_clear_issue_category() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        for _ in 0..14 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        }
        app.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        let popup = app.popup_components.back().expect("popup should be open");
        match &*popup.borrow() {
            PopupComponent::SelectBox(select_box) => {
                let widget = select_box.create_widget();
                assert_eq!(widget.items[0], (None, "選択なし(None)".to_string()));
                assert_eq!(widget.focused_index, 1);
            }
            PopupComponent::SpentTimeInput(_) => panic!("category popup should be a select box"),
            _ => {}
        }

        app.process_event(key_event(KeyCode::Char('k')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone());
        dispatcher.borrow_mut().consume_action();

        let issue_category_id = dispatcher
            .borrow()
            .store()
            .get_issue(3)
            .map(|(issue, _)| issue.category_id);
        assert_eq!(issue_category_id, Some(None));
        assert!(app.popup_components.is_empty());
    }

    #[test]
    fn start_date_property_opens_date_picker_and_updates_store_through_dispatcher() {
        let dispatcher = dispatcher_with_issue();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));

        focus_property_line(&mut app, dispatcher.clone(), 9);
        app.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert_eq!(app.popup_components.len(), 1);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::DatePicker(_)
        ));

        select_next_day_in_open_date_picker(&mut app, dispatcher.clone());
        assert_eq!(app.popup_components.len(), 0);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);

        dispatcher.borrow_mut().consume_action();
        let selected = dispatcher
            .borrow()
            .store()
            .get_issue(3)
            .unwrap()
            .0
            .start_date;
        assert_eq!(
            selected,
            Some(crate::test_support::local_datetime(
                "2026-02-17T00:00:00+09:00"
            ))
        );
    }

    #[test]
    fn due_date_property_opens_date_picker_and_updates_store_through_dispatcher() {
        let dispatcher = dispatcher_with_issue();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));

        focus_property_line(&mut app, dispatcher.clone(), 10);
        app.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        assert_eq!(app.popup_components.len(), 1);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::DatePicker(_)
        ));

        select_next_day_in_open_date_picker(&mut app, dispatcher.clone());
        assert_eq!(app.popup_components.len(), 0);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 1);

        dispatcher.borrow_mut().consume_action();
        let selected = dispatcher.borrow().store().get_issue(3).unwrap().0.due_date;
        assert_eq!(
            selected,
            Some(crate::test_support::local_datetime(
                "2026-02-18T00:00:00+09:00"
            ))
        );
    }

    #[test]
    fn e_key_on_journal_notes_opens_editor_and_updates_store_through_dispatcher() {
        let dispatcher = loaded_dispatcher_with_journals();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        focus_first_journal_notes(&mut app, dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        let Some(AppEffect::OpenEditor(request)) = app.take_effect() else {
            panic!("Journal本文編集時はエディタ起動effectが必要です");
        };
        assert_eq!(request.initial_text, "");

        app.handle_editor_response(EditorResponse {
            edited_text: "updated notes".to_string(),
        });
        dispatcher.borrow_mut().consume_action();

        let dispatcher_ref = dispatcher.borrow();
        let entry = dispatcher_ref.store().get_remote_journal(3, 1);
        let RemoteJournalState::Edited { diff, failure } = &entry.state else {
            panic!("expected edited state");
        };
        assert_eq!(diff.before, "");
        assert_eq!(diff.after, "updated notes");
        assert!(failure.is_none());
    }

    #[test]
    fn y_key_from_issue_component_opens_issue_select_popup() {
        let dispatcher = loaded_dispatcher_with_edited_issue();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        app.process_event(key_event(KeyCode::Char('y')), dispatcher.clone());

        assert_eq!(app.popup_components.len(), 1);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::IssueSelect(_)
        ));
        assert!(matches!(
            app.take_effect(),
            Some(AppEffect::FetchProjectIssuesPage { project_id, page, .. })
                if project_id == 1 && page == NonZeroUsize::MIN
        ));
    }

    #[test]
    fn q_key_closes_open_issue_select_popup() {
        let dispatcher = loaded_dispatcher_with_edited_issue();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        let popup = IssueSelectPopupComponent::new(dispatcher.borrow().store(), Some(3.into()));
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(popup))));
        assert_eq!(app.popup_components.len(), 1);
        app.process_event(key_event(KeyCode::Char('q')), dispatcher.clone());

        assert!(app.popup_components.is_empty());
    }

    /// updateでキャッシュしたプレビューがrenderで実際に描画されることを確認する
    #[test]
    fn snapshot_issue_select_popup_preview_is_rendered_after_update() {
        let dispatcher = dispatcher_with_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        let popup_component = {
            let dispatcher_ref = dispatcher.borrow();
            IssueSelectPopupComponent::new(dispatcher_ref.store(), Some(IssueId::new(3)))
        };
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(
                popup_component,
            ))));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        crate::test_support::render_frame_snapshot(
            "app_issue_select_popup_preview_after_update",
            AREA.width,
            AREA.height,
            |frame| app.render(dispatcher.borrow().store(), frame, AREA),
        );
    }

    #[test]
    fn enter_on_different_issue_in_issue_select_popup_replaces_issue_detail_component() {
        let dispatcher = dispatcher_with_edited_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        app.open_issue_select_popup(Some(3.into()));
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('l')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('k')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(app.popup_components.is_empty());
        assert_eq!(app.issue_component.unwrap().issue_id(), 1);
    }

    #[test]
    fn selecting_the_same_issue_removes_popup_and_replaces_issue_component() {
        let dispatcher = dispatcher_with_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        let initial_cursor = app.cursor_position(dispatcher.borrow().store(), AREA);

        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        assert_ne!(
            app.cursor_position(dispatcher.borrow().store(), AREA),
            initial_cursor,
            "test setup must move the old component focus"
        );
        app.process_event(key_event(KeyCode::Char('y')), dispatcher.clone());
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('l')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone());
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        assert!(app.popup_components.is_empty());
        assert_eq!(app.issue_component.as_ref().unwrap().issue_id(), 3);
        assert_eq!(
            app.cursor_position(dispatcher.borrow().store(), AREA),
            initial_cursor,
            "same-ID selection must construct a fresh component"
        );
        assert!(app.take_effect().is_none());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn selecting_unknown_issue_removes_popup_replaces_component_and_installs_fetch_effect() {
        let dispatcher = dispatcher_with_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        app.open_issue_select_popup(Some(3.into()));
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone());

        app.process_event(key_event(KeyCode::Char('l')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone());

        assert!(app.popup_components.is_empty());
        assert_eq!(app.issue_component.as_ref().unwrap().issue_id(), 42);
        assert!(matches!(app.take_effect(), Some(AppEffect::FetchIssue(id)) if id == 42));
        assert!(app.take_effect().is_none());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn startup_issue_select_popup_exposes_its_initial_page_fetch_effect_once() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher, None);

        assert!(matches!(
            app.take_effect(),
            Some(AppEffect::FetchProjectIssuesPage { project_id, page, .. })
                if project_id == 1 && page == std::num::NonZeroUsize::MIN
        ));
        assert!(app.take_effect().is_none());
    }

    #[test]
    fn startup_without_initial_issue_selects_first_project_even_when_issue_store_has_other_projects()
     {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_fixture_entity_actions(&mut dispatcher_ref);
            let mut lower_id_issue = crate::test_support::sample_issue_aggregate(
                1,
                "project two issue",
                1.into(),
                None,
                None,
                None,
                0,
            );
            lower_id_issue.issue.project_id = 2.into();
            let higher_id_issue = crate::test_support::sample_issue_aggregate(
                3,
                "project one issue",
                1.into(),
                None,
                None,
                None,
                0,
            );
            dispatcher_ref.dispatch(IssueAction::Sync {
                issue: lower_id_issue,
            });
            dispatcher_ref.dispatch(IssueAction::Sync {
                issue: higher_id_issue,
            });
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
        }

        let mut app = AppComponent::new(dispatcher, None);

        assert!(matches!(
            app.take_effect(),
            Some(AppEffect::FetchProjectIssuesPage { project_id, page, .. })
                if project_id == ProjectId::new(1) && page == NonZeroUsize::MIN
        ));
    }

    #[test]
    fn startup_issue_select_popup_exposes_fetch_without_dispatching_project_issues_action() {
        let dispatcher = dispatcher_with_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), None);

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert!(matches!(
            app.take_effect(),
            Some(AppEffect::FetchProjectIssuesPage { project_id, page })
                if project_id == ProjectId::new(1)
                    && page == NonZeroUsize::MIN
        ));
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn q_closes_issue_select_popup_without_dispatching_project_issues_action() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone(), None);
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone());

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone());

        assert!(should_continue);
        assert!(app.popup_components.is_empty());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn q_without_popup_requests_application_exit() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));

        assert!(!app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher));
    }
}
