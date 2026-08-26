use std::cell::RefCell;
use std::collections::VecDeque;
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
use crate::components::issue_select_popup::component::IssueSelectPopupComponent;
use crate::stores::{Dispatcher, IssueAction, Store};
use crate::usecases::redmine::{cancel_issue_upload, continue_issue_upload};
use crate::vos::{
    CategoryId, EntityIdValue, IssueId, IssuePropertyDiff, IssueStatusId, TargetVersionId,
    TimeEntityActivityId, UserId,
};

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
    OpenEditor(EditorRequest),
    StartIssueUpload(IssueId),
    ContinueIssueUpload {
        id: IssueId,
        diffs: Vec<IssuePropertyDiff>,
    },
}

enum PendingEditorContext {
    IssueBody { id: IssueId },
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
        let popup_components = if issue_id.is_none() {
            let popup_component = {
                let dispatcher = dispatcher.borrow();
                let focused_issue_id = dispatcher.store().get_issues().keys().min().copied();
                IssueSelectPopupComponent::new(dispatcher.store(), focused_issue_id)
            };
            VecDeque::from([Rc::new(RefCell::new(PopupComponent::IssueSelect(
                popup_component,
            )))])
        } else {
            VecDeque::new()
        };
        let mut app = AppComponent {
            issue_component,
            popup_components,
            dispatcher,
            pending_effect: None,
            pending_editor_context: None,
        };
        if let Some(result) = issue_result {
            app.handle_issue_component_result(result);
        }
        app
    }

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
    pub fn process_event(&mut self, event: Event, dispatcher: Rc<RefCell<Dispatcher>>) {
        if let Some(popup_component) = self.popup_components.back() {
            let popup_component_rc = popup_component.clone();
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
                        Some(
                            SpentTimeInputPopupEventProcessResult::OpenTimeEntityActivitiesPopup,
                        ) => {
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
                    let result = popup_component.process_event(event);
                    match result {
                        Some(IssueSelectPopupEventProcessResult::Selected { issue_id }) => {
                            self.select_issue(issue_id);
                        }
                        Some(IssueSelectPopupEventProcessResult::Quited) => {
                            self.popup_components.pop_back();
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
        } else if self.issue_component.is_some() {
            let (result, issue_id) = {
                let issue_component = self
                    .issue_component
                    .as_mut()
                    .expect("checked that IssueComponent exists");
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
                    let popup =
                        IssueSelectPopupComponent::new(dispatcher.borrow().store(), Some(issue_id));
                    self.popup_components
                        .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(popup))));
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
                    let issue_statuses = dispatcher
                        .borrow()
                        .store()
                        .get_issue_statuses()
                        .iter()
                        .map(|(id, status)| (id.get(), status.name.clone()))
                        .collect::<Vec<_>>();
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &issue_statuses,
                            0,
                            false,
                            Box::new(move |status_id| {
                                if let Some(status_id) = status_id {
                                    dispatcher.borrow_mut().dispatch(IssueAction::UpdateStatus {
                                        id: issue_id,
                                        status_id: IssueStatusId::new(status_id),
                                    });
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::Detail(
                    IssueDetailEventProcessResult::OpenAssignedToPopup,
                )) => {
                    let dispatcher_ref = dispatcher.borrow();
                    let store = dispatcher_ref.store();
                    let current_assigned_to_id = store
                        .get_issue(issue_id)
                        .and_then(|(issue, _)| issue.assigned_to_id);
                    let mut users = store
                        .get_users()
                        .iter()
                        .map(|(id, user)| (id.get(), user.name.clone()))
                        .collect::<Vec<_>>();
                    users.sort_by_key(|(id, _)| *id);
                    let focused_index = current_assigned_to_id
                        .and_then(|current_id| {
                            users.iter().position(|(id, _)| *id == current_id.get())
                        })
                        .unwrap_or(0);
                    drop(dispatcher_ref);

                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &users,
                            focused_index,
                            true,
                            Box::new(move |assigned_to_id| {
                                dispatcher
                                    .borrow_mut()
                                    .dispatch(IssueAction::UpdateAssignedTo {
                                        id: issue_id.into(),
                                        assigned_to_id: assigned_to_id.map(UserId::new),
                                    });
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::Detail(
                    IssueDetailEventProcessResult::OpenTargetVersionPopup,
                )) => {
                    let dispatcher_ref = dispatcher.borrow();
                    let store = dispatcher_ref.store();
                    let current_target_version_id = store
                        .get_issue(issue_id)
                        .and_then(|(issue, _)| issue.target_version_id);
                    let mut target_versions = store
                        .get_target_versions()
                        .iter()
                        .map(|(id, target_version)| (id.get(), target_version.name.clone()))
                        .collect::<Vec<_>>();
                    target_versions.sort_by_key(|(id, _)| *id);
                    let focused_index = current_target_version_id
                        .and_then(|current_id| {
                            target_versions
                                .iter()
                                .position(|(id, _)| *id == current_id.get())
                        })
                        .unwrap_or(0);
                    drop(dispatcher_ref);

                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &target_versions,
                            focused_index,
                            true,
                            Box::new(move |target_version_id| {
                                dispatcher.borrow_mut().dispatch(
                                    IssueAction::UpdateTargetVersion {
                                        id: issue_id.into(),
                                        target_version_id: target_version_id
                                            .map(TargetVersionId::new),
                                    },
                                );
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::Detail(
                    IssueDetailEventProcessResult::OpenStartDatePopup,
                )) => {
                    let selected_date = dispatcher
                        .borrow()
                        .store()
                        .get_issue(issue_id)
                        .and_then(|(issue, _)| issue.start_date);

                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::DatePicker(DatePickerPopupComponent::new(
                            selected_date,
                            Box::new(move |date| {
                                if let Some(date) = date {
                                    dispatcher.borrow_mut().dispatch(
                                        IssueAction::UpdateStartDate {
                                            id: issue_id.into(),
                                            start_date: Some(date),
                                        },
                                    );
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::Detail(
                    IssueDetailEventProcessResult::OpenDueDatePopup,
                )) => {
                    let selected_date = dispatcher
                        .borrow()
                        .store()
                        .get_issue(issue_id)
                        .and_then(|(issue, _)| issue.due_date);

                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::DatePicker(DatePickerPopupComponent::new(
                            selected_date,
                            Box::new(move |date| {
                                if let Some(date) = date {
                                    dispatcher
                                        .borrow_mut()
                                        .dispatch(IssueAction::UpdateDueDate {
                                            id: issue_id.into(),
                                            due_date: Some(date),
                                        });
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::Detail(
                    IssueDetailEventProcessResult::OpenDoneRatioPopup,
                )) => {
                    let current_done_ratio = dispatcher
                        .borrow()
                        .store()
                        .get_issue(issue_id)
                        .map(|(issue, _)| issue.done_ratio)
                        .unwrap_or(0);
                    let done_ratios = (0..=100)
                        .step_by(10)
                        .map(|ratio| (ratio, ratio.to_string()))
                        .collect::<Vec<_>>();
                    let focused_index = done_ratios
                        .iter()
                        .position(|(ratio, _)| *ratio == current_done_ratio)
                        .unwrap_or(0);

                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &done_ratios,
                            focused_index,
                            false,
                            Box::new(move |done_ratio| {
                                if let Some(done_ratio) = done_ratio {
                                    dispatcher.borrow_mut().dispatch(
                                        IssueAction::UpdateDoneRatio {
                                            id: issue_id.into(),
                                            done_ratio,
                                        },
                                    );
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::Detail(
                    IssueDetailEventProcessResult::OpenCategoryPopup,
                )) => {
                    let dispatcher_ref = dispatcher.borrow();
                    let store = dispatcher_ref.store();
                    let current_category_id = store
                        .get_issue(issue_id)
                        .and_then(|(issue, _)| issue.category_id);
                    let mut categories = store
                        .get_categories()
                        .iter()
                        .map(|(id, category)| (id.get(), category.name.clone()))
                        .collect::<Vec<_>>();
                    categories.sort_by_key(|(id, _)| *id);
                    let focused_index = current_category_id
                        .and_then(|current_id| {
                            categories
                                .iter()
                                .position(|(id, _)| *id == current_id.get())
                        })
                        .unwrap_or(0);
                    drop(dispatcher_ref);

                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &categories,
                            focused_index,
                            true,
                            Box::new(move |category_id| {
                                dispatcher
                                    .borrow_mut()
                                    .dispatch(IssueAction::UpdateCategory {
                                        id: issue_id.into(),
                                        category_id: category_id.map(CategoryId::new),
                                    });
                            }),
                        )),
                    )));
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
                None => {}
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::vos::IssuePropertyDiff;
    use crate::vos::issue_property_diff::IssueDescriptionDiff;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

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
        server_issue.description = "server body".to_string();
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
        }
        dispatcher
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

        let popup = IssueSelectPopupComponent::new(dispatcher.borrow().store(), Some(3.into()));
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(popup))));
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
    }

    #[test]
    fn selecting_unknown_issue_removes_popup_replaces_component_and_installs_fetch_effect() {
        let dispatcher = dispatcher_with_selectable_issues();
        let mut app = AppComponent::new(dispatcher.clone(), Some(3.into()));
        let popup = IssueSelectPopupComponent::new(dispatcher.borrow().store(), Some(3.into()));
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(popup))));

        app.select_issue(IssueId::new(42));

        assert!(app.popup_components.is_empty());
        assert_eq!(app.issue_component.as_ref().unwrap().issue_id(), 42);
        assert!(matches!(app.take_effect(), Some(AppEffect::FetchIssue(id)) if id == 42));
        assert!(app.take_effect().is_none());
    }
}
