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
use crate::vos::{
    CategoryId, EntityIdValue, IssueStatusId, TargetVersionId, TimeEntityActivityId, UserId,
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
    OpenEditor(EditorRequest),
}

enum PendingEditorContext {
    IssueBody { id: u16 },
}

enum PopupComponent<'a> {
    SelectBox(SelectBoxPopupComponent<'a>),
    SpentTimeInput(SpentTimeInputPopupComponent<'a>),
    DatePicker(DatePickerPopupComponent<'a>),
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
                Some(IssueEventProcessResult::OpenTargetVersionPopup) => {
                    let dispatcher_ref = dispatcher.borrow();
                    let store = dispatcher_ref.store();
                    let current_target_version_id = store
                        .get_issue(self.issue_component.id)
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

                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &target_versions,
                            focused_index,
                            true,
                            Box::new(move |target_version_id| {
                                dispatcher.borrow_mut().dispatch(
                                    Action::UpdateIssueTargetVersion {
                                        id: issue_id,
                                        target_version_id: target_version_id
                                            .map(TargetVersionId::new),
                                    },
                                );
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::OpenStartDatePopup) => {
                    let selected_date = dispatcher
                        .borrow()
                        .store()
                        .get_issue(self.issue_component.id)
                        .and_then(|(issue, _)| issue.start_date);

                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::DatePicker(DatePickerPopupComponent::new(
                            selected_date,
                            Box::new(move |date| {
                                if let Some(date) = date {
                                    dispatcher.borrow_mut().dispatch(
                                        Action::UpdateIssueStartDate {
                                            id: issue_id,
                                            start_date: Some(date),
                                        },
                                    );
                                }
                            }),
                        )),
                    )));
                }
                Some(IssueEventProcessResult::OpenDueDatePopup) => {
                    let selected_date = dispatcher
                        .borrow()
                        .store()
                        .get_issue(self.issue_component.id)
                        .and_then(|(issue, _)| issue.due_date);

                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::DatePicker(DatePickerPopupComponent::new(
                            selected_date,
                            Box::new(move |date| {
                                if let Some(date) = date {
                                    dispatcher
                                        .borrow_mut()
                                        .dispatch(Action::UpdateIssueDueDate {
                                            id: issue_id,
                                            due_date: Some(date),
                                        });
                                }
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
                Some(IssueEventProcessResult::OpenCategoryPopup) => {
                    let dispatcher_ref = dispatcher.borrow();
                    let store = dispatcher_ref.store();
                    let current_category_id = store
                        .get_issue(self.issue_component.id)
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

                    let issue_id = self.issue_component.id;
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::SelectBox(SelectBoxPopupComponent::new(
                            &categories,
                            focused_index,
                            true,
                            Box::new(move |category_id| {
                                dispatcher
                                    .borrow_mut()
                                    .dispatch(Action::UpdateIssueCategory {
                                        id: issue_id,
                                        category_id: category_id.map(CategoryId::new),
                                    });
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
                PopupComponent::DatePicker(popup_component) => {
                    let widget = popup_component.create_widget();
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

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn loaded_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            dispatcher_ref.dispatch(Action::LoadUsers);
            dispatcher_ref.dispatch(Action::LoadIssueStatuses);
            dispatcher_ref.dispatch(Action::LoadPriorities);
            dispatcher_ref.dispatch(Action::LoadProjects);
            dispatcher_ref.dispatch(Action::LoadTrackers);
            dispatcher_ref.dispatch(Action::LoadTargetVersions);
            dispatcher_ref.dispatch(Action::LoadCategories);
            dispatcher_ref.dispatch(Action::LoadIssue { id: 3 });
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
        }
        dispatcher
    }

    fn dispatcher_with_issue() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        dispatcher
            .borrow_mut()
            .dispatch(Action::LoadIssue { id: 3 });
        dispatcher.borrow_mut().consume_action();
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
    fn category_popup_includes_none_and_can_clear_issue_category() {
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(dispatcher.clone());
        app.update(dispatcher.clone(), dispatcher.borrow().store());

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
        let mut app = AppComponent::new(dispatcher.clone());

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
        let mut app = AppComponent::new(dispatcher.clone());

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
}
