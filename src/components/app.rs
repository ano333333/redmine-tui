use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Action, Dispatcher, Store};
use crate::components::issue::{
    EventProcessResult as IssueEventProcessResult, IssueDetailComponent,
};
use crate::entities::IssueStatusId;

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

enum PopupComponent {
    SelectBox(SelectBoxPopupComponent),
    SpentTimeInput(SpentTimeInputPopupComponent),
}

pub struct AppComponent {
    issue_component: IssueDetailComponent,
    // popup追加の際は末尾に追加する、先頭要素が最奥に表示される
    popup_components: VecDeque<PopupComponent>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pending_effect: Option<AppEffect>,
    pending_editor_context: Option<PendingEditorContext>,
}

impl AppComponent {
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
        if let Some(popup_component) = &mut self.popup_components.back_mut() {
            match popup_component {
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
                    self.popup_components.push_back(PopupComponent::SelectBox(
                        SelectBoxPopupComponent::new(
                            &issue_statuses,
                            0,
                            Box::new(move |status_id| {
                                dispatcher.borrow_mut().dispatch(Action::UpdateIssueStatus {
                                    id: issue_id,
                                    status_id: IssueStatusId::new(status_id),
                                });
                            }),
                        ),
                    ));
                }
                Some(IssueEventProcessResult::OpenSpentTimeInputPopup) => {
                    self.popup_components
                        .push_back(PopupComponent::SpentTimeInput(
                            SpentTimeInputPopupComponent::new(dispatcher.borrow().store()),
                        ));
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
        if self.popup_components.is_empty() {
            frame.set_cursor_position(self.issue_component.calc_cursor_position(store, area));
        }
        self.render_popup_component(frame, area, store);
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
            match popup_component {
                PopupComponent::SelectBox(popup_component) => {
                    let widget = popup_component.create_widget();
                    frame.render_widget(widget, area);
                }
                PopupComponent::SpentTimeInput(popup_component) => {
                    let widget = popup_component.create_widget(store);
                    frame.render_widget(widget, area);
                    if let Some(cursor_position) = popup_component.cursor_position(area) {
                        frame.set_cursor_position(cursor_position);
                    }
                }
            }
        }
    }
}
