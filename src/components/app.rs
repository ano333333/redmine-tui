use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};

use crate::app::{Action, Dispatcher, Store};
use crate::components::issue::{
    EventProcessResult as IssueEventProcessResult, IssueDetailComponent,
};
use crate::entities::{EntityIdValue, IssueStatusId, TimeEntityActivityId, UserId};

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
    OpenEditor(EditorRequest),
}

enum PendingEditorContext {
    IssueBody { id: u16 },
}

enum PopupComponent<'a> {
    SelectBox(SelectBoxPopupComponent<'a>),
    SpentTimeInput(SpentTimeInputPopupComponent<'a>),
}

pub struct AppComponent<'a> {
    issue_component: IssueDetailComponent,
    // popup追加の際は末尾に追加する、先頭要素が最奥に表示される
    popup_components: VecDeque<Rc<RefCell<PopupComponent<'a>>>>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pending_effect: Option<AppEffect>,
    pending_editor_context: Option<PendingEditorContext>,
}

impl<'a> AppComponent<'a> {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        AppComponent {
            issue_component: IssueDetailComponent::new(dispatcher.clone(), 3),
            popup_components: VecDeque::new(),
            dispatcher,
            pending_effect: None,
            pending_editor_context: None,
        }
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
            }
        } else {
            let result = self
                .issue_component
                .process_event(event, self.dispatcher.clone());
            match result {
                Some(IssueEventProcessResult::EditIssueBodyRequested { id, body }) => {
                    self.pending_editor_context = Some(PendingEditorContext::IssueBody { id });
                    self.pending_effect =
                        Some(AppEffect::OpenEditor(EditorRequest { initial_text: body }));
                }
                Some(IssueEventProcessResult::OpenIssueStatusPopup) => {
                    let issue_statuses = dispatcher
                        .borrow()
                        .store()
                        .get_issue_statuses()
                        .iter()
                        .map(|(id, status)| (*id, status.name.clone()))
                        .collect::<Vec<_>>();
                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &issue_statuses,
                            0,
                            false,
                            Box::new(move |status_id| {
                                if let Some(status_id) = status_id {
                                    dispatcher.borrow_mut().dispatch(Action::UpdateIssueStatus {
                                        id: issue_id,
                                        status_id: IssueStatusId::new(status_id),
                                    });
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::OpenAssignedToPopup) => {
                    let dispatcher_ref = dispatcher.borrow();
                    let store = dispatcher_ref.store();
                    let current_assigned_to_id = store
                        .get_issue(self.issue_component.id)
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

                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &users,
                            focused_index,
                            true,
                            Box::new(move |assigned_to_id| {
                                dispatcher
                                    .borrow_mut()
                                    .dispatch(Action::UpdateIssueAssignedTo {
                                        id: issue_id,
                                        assigned_to_id: assigned_to_id.map(UserId::new),
                                    });
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::OpenDoneRatioPopup) => {
                    let current_done_ratio = dispatcher
                        .borrow()
                        .store()
                        .get_issue(self.issue_component.id)
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

                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &done_ratios,
                            focused_index,
                            false,
                            Box::new(move |done_ratio| {
                                if let Some(done_ratio) = done_ratio {
                                    dispatcher.borrow_mut().dispatch(
                                        Action::UpdateIssueDoneRatio {
                                            id: issue_id,
                                            done_ratio,
                                        },
                                    );
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::OpenSpentTimeInputPopup) => {
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SpentTimeInput(SpentTimeInputPopupComponent::new(
                            dispatcher.borrow().store(),
                        )),
                    )));
                }
                None => {}
            }
        }
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        self.issue_component.update(dispatcher, store);
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, area: Rect) {
        self.issue_component.render(store, frame, area);
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
                    .dispatch(crate::app::Action::UpdateIssue {
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
            }
        } else {
            Some(self.issue_component.calc_cursor_position(store, area))
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
