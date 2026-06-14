use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;

use crate::app::{Action, Dispatcher, IssueStatusState, Store};
use crate::components::issue::{
    EventProcessResult as IssueEventProcessResult, IssueDetailComponent,
};

use super::select_box_popup::{
    EventProcessResult as SelectBoxPopupEventProcessResult, SelectBoxPopupComponent,
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

pub struct AppComponent {
    issue_component: IssueDetailComponent,
    popup_component: Option<SelectBoxPopupComponent>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pending_effect: Option<AppEffect>,
    pending_editor_context: Option<PendingEditorContext>,
}

impl AppComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        AppComponent {
            issue_component: IssueDetailComponent::new(dispatcher.clone(), 3),
            popup_component: None,
            dispatcher,
            pending_effect: None,
            pending_editor_context: None,
        }
    }

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
    pub fn process_event(&mut self, event: Event, dispatcher: Rc<RefCell<Dispatcher>>) {
        if let Some(popup_component) = &mut self.popup_component {
            let result = popup_component.process_event(event, self.dispatcher.clone());
            match result {
                Some(SelectBoxPopupEventProcessResult::Entered)
                | Some(SelectBoxPopupEventProcessResult::Quited) => {
                    self.popup_component = None;
                }
                None => {}
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
                    if self.popup_component.is_some() {
                        return;
                    }
                    let issue_statuses = dispatcher
                        .borrow()
                        .store()
                        .get_issue_statuses()
                        .iter()
                        .filter(|(_, (_, state))| *state == IssueStatusState::Existing)
                        .map(|(id, (status, _))| (*id, status.name.clone()))
                        .collect::<Vec<_>>();
                    let issue_id = self.issue_component.id;
                    self.popup_component = Some(SelectBoxPopupComponent::new(
                        &issue_statuses,
                        0,
                        Box::new(move |status_id| {
                            dispatcher.borrow_mut().dispatch(Action::UpdateIssueStatus {
                                id: issue_id,
                                status_id,
                            });
                        }),
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
        self.render_popup_component(frame, area);
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

    fn render_popup_component(&self, frame: &mut Frame, area: Rect) {
        if let Some(popup_component) = &self.popup_component {
            let widget = popup_component.create_widget();
            let vert_layouts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Percentage(50),
                    Constraint::Fill(1),
                ])
                .split(area);
            let hor_layouts = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Percentage(50),
                    Constraint::Fill(1),
                ])
                .split(vert_layouts[1]);
            let area = hor_layouts[1];

            frame.render_widget(Clear, area);
            frame.render_widget(widget, area);
        }
    }
}
