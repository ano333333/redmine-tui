use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Dispatcher, Store};
use crate::components::issue::{
    EventProcessResult as IssueEventProcessResult, IssueDetailComponent,
};

use super::select_box_popup::SelectBoxPopupComponent;

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
    popup: Option<SelectBoxPopupComponent>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pending_effect: Option<AppEffect>,
    pending_editor_context: Option<PendingEditorContext>,
}

impl AppComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        AppComponent {
            issue_component: IssueDetailComponent::new(dispatcher.clone(), 3),
            popup: None,
            dispatcher,
            pending_effect: None,
            pending_editor_context: None,
        }
    }

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
    pub fn process_event(&mut self, event: Event) {
        let result = self
            .issue_component
            .process_event(event, self.dispatcher.clone());
        match result {
            Some(IssueEventProcessResult::EditIssueBodyRequested { id, body }) => {
                self.pending_editor_context = Some(PendingEditorContext::IssueBody { id });
                self.pending_effect =
                    Some(AppEffect::OpenEditor(EditorRequest { initial_text: body }));
            }
            None => {}
        }
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        self.issue_component.update(dispatcher, store);
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, area: Rect) {
        self.issue_component.render(store, frame, area);
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
}
