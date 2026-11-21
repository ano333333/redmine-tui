use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;

use crate::components::RequestSink;
use crate::components::issue::{
    EventProcessResult as IssueEventProcessResult, IssueComponent, IssueDetailEventProcessResult,
};
use crate::components::issue_property_conflict_popup::{
    EventProcessResult as IssuePropertyConflictEventProcessResult, IssuePropertyConflictComponent,
};
use crate::components::issue_select_popup::component::EventProcessResult as IssueSelectPopupEventProcessResult;
use crate::components::issue_select_popup::component::IssueSelectPopupComponent;
use crate::components::new_issue::{
    DraftField, EventProcessResult as NewIssueEventProcessResult, NewIssueComponent,
};
use crate::components::remote_journal_conflict_popup::{
    EventProcessResult as RemoteJournalConflictEventProcessResult, RemoteJournalConflictComponent,
};
use crate::platform::editor::{EditorOutcome, EditorRequest, InteractionMode};
use crate::platform::host::CursorRendering;
use crate::platform::input::{InputEvent, KeyCode};
use crate::stores::{
    Action, DeletedJournalState, Dispatcher, IssueAction, IssueState, JournalAction,
    LocalJournalState, RemoteJournalState, Store,
};
use crate::usecases::issue_popup_options::{
    assigned_to_popup_observer, build_assigned_to_options, build_category_options,
    build_done_ratio_options, build_issue_status_options, build_priority_options,
    build_project_options, build_target_version_options, build_tracker_options,
    category_popup_observer, current_due_date, current_estimated_hours, current_start_date,
    done_ratio_popup_observer, due_date_popup_observer, estimated_hours_popup_observer,
    issue_status_popup_observer, priority_popup_observer, project_popup_observer,
    start_date_popup_observer, target_version_popup_observer, tracker_popup_observer,
};
use crate::usecases::{
    DraftAttachment, IssueCreationId, NewIssueDraft, UsecaseEvent, UsecaseFailure, UsecaseRequest,
};
use crate::vos::{EntityIdValue, IssueId, JournalId, TimeEntityActivityId};

use super::attachment_input_popup::{
    AttachmentInputPopupComponent, EventProcessResult as AttachmentInputPopupEventProcessResult,
};
use super::date_picker_popup::component::{
    DatePickerPopupComponent, EventProcessResult as DatePickerPopupEventProcessResult,
};
use super::number_input_popup::{
    EventProcessResult as NumberInputPopupEventProcessResult, NumberInputPopupComponent,
};
use super::select_box_popup::{
    EventProcessResult as SelectBoxPopupEventProcessResult, SelectBoxPopupComponent,
};
use super::spent_time_input_popup::{
    EventProcessResult as SpentTimeInputPopupEventProcessResult, SpentTimeInputPopupComponent,
};
use super::toast_list::ToastListComponent;

enum PendingEditorContext {
    IssueBody {
        id: IssueId,
    },
    RemoteJournal {
        issue_id: IssueId,
        journal_id: JournalId,
    },
    LocalJournal {
        issue_id: IssueId,
    },
    DeletedJournal {
        issue_id: IssueId,
        original_id: JournalId,
    },
    NewIssueSubject {
        creation_id: IssueCreationId,
    },
    NewIssueDescription {
        creation_id: IssueCreationId,
    },
}

/// Storeの状態遷移違反とeditor sessionの二重起動を防ぐため、editorの起動を要求できる状態かを最終確認する。
///
/// 各Componentの操作可否判定はUIイベントを抑制する責務として残し、この境界でも検査する。
fn can_start_editing(
    context: &PendingEditorContext,
    interaction_mode: InteractionMode,
    store: &Store,
) -> bool {
    if interaction_mode == InteractionMode::Editing {
        return false;
    }
    match context {
        PendingEditorContext::IssueBody { id } => {
            !matches!(store.try_get_issue_state(*id), Some(IssueState::Uploading))
        }
        PendingEditorContext::RemoteJournal {
            issue_id,
            journal_id,
        } => !matches!(
            store.get_remote_journal(*issue_id, *journal_id).state,
            RemoteJournalState::Uploading { .. }
        ),
        PendingEditorContext::LocalJournal { issue_id } => matches!(
            store.try_get_local_journal(*issue_id),
            Some(entry) if matches!(entry.state, LocalJournalState::LocalOnly { .. })
        ),
        PendingEditorContext::DeletedJournal {
            issue_id,
            original_id,
        } => matches!(
            store.get_deleted_journal(*issue_id, *original_id).state,
            DeletedJournalState::Pending { .. }
        ),
        PendingEditorContext::NewIssueSubject { .. }
        | PendingEditorContext::NewIssueDescription { .. } => true,
    }
}

/// 題名は1行のため、editorで入力した改行と連続する空白を1つの空白にまとめ、前後の空白を除く。
fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 添付入力popupで入力したファイルの送り先。
enum AttachmentDestination {
    /// 取得済みのIssueへすぐに添付する。
    Issue(IssueId),
    /// 下書きに積み、作成と同時に添付する。
    Draft(Rc<RefCell<NewIssueDraft>>),
}

enum PopupComponent<'a> {
    AttachmentInput {
        destination: AttachmentDestination,
        component: AttachmentInputPopupComponent<'a>,
    },
    SelectBox(SelectBoxPopupComponent<'a>),
    SpentTimeInput(SpentTimeInputPopupComponent<'a>),
    DatePicker(DatePickerPopupComponent<'a>),
    NumberInput(NumberInputPopupComponent<'a>),
    IssueSelect(IssueSelectPopupComponent),
    IssuePropertyConflict {
        issue_id: IssueId,
        component: IssuePropertyConflictComponent,
    },
    RemoteJournalConflict {
        issue_id: IssueId,
        journal_id: JournalId,
        component: RemoteJournalConflictComponent,
    },
}

/// popupの下に表示する画面。
enum MainComponent {
    Issue(IssueComponent),
    NewIssue(NewIssueComponent),
}

pub struct AppComponent<'a> {
    main_component: Option<MainComponent>,
    // popup追加の際は末尾に追加する、先頭要素が最奥に表示される
    popup_components: VecDeque<Rc<RefCell<PopupComponent<'a>>>>,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pending_editor_context: Option<PendingEditorContext>,
    interaction_mode: InteractionMode,
    cursor_rendering: CursorRendering,
    toast_list: ToastListComponent,
}

impl<'a> AppComponent<'a> {
    pub fn new(
        dispatcher: Rc<RefCell<Dispatcher>>,
        issue_id: Option<IssueId>,
        cursor_rendering: CursorRendering,
        sink: &mut RequestSink,
    ) -> Self {
        let mut app = AppComponent {
            main_component: None,
            popup_components: VecDeque::new(),
            dispatcher,
            pending_editor_context: None,
            interaction_mode: InteractionMode::Application,
            cursor_rendering,
            toast_list: ToastListComponent::default(),
        };
        match issue_id {
            Some(issue_id) => app.open_issue(issue_id, sink),
            None => app.open_issue_select_popup(None, sink),
        }
        app
    }

    /// 共通の入力イベントを同期的に処理する。updateとrenderがこの順で後続する。
    /// イベントを使った場合にtrueを返す。
    pub fn process_event(
        &mut self,
        event: InputEvent,
        dispatcher: Rc<RefCell<Dispatcher>>,
        sink: &mut RequestSink,
    ) -> bool {
        // editor session中の入力はeditorが占有するため、Componentの状態を変更しない。
        if self.interaction_mode == InteractionMode::Editing {
            return false;
        }
        if self.popup_components.back().is_some() {
            return self.process_popup_event(event, dispatcher, sink);
        }
        match &self.main_component {
            Some(MainComponent::Issue(_)) => self.process_issue_event(event, dispatcher, sink),
            Some(MainComponent::NewIssue(_)) => self.process_new_issue_event(event, sink),
            None => false,
        }
    }

    /// 最前面のpopupへイベントを渡し、結果に応じてpopup stackを更新する。
    /// popupがイベントを使った場合にtrueを返す。
    fn process_popup_event(
        &mut self,
        event: InputEvent,
        dispatcher: Rc<RefCell<Dispatcher>>,
        sink: &mut RequestSink,
    ) -> bool {
        let popup_component_rc = self
            .popup_components
            .back()
            .expect("caller checked that a popup exists")
            .clone();
        match &mut *(popup_component_rc.borrow_mut()) {
            PopupComponent::AttachmentInput {
                destination,
                component,
            } => {
                let Some(result) = component.process_event(event) else {
                    return false;
                };
                match result {
                    AttachmentInputPopupEventProcessResult::Entered { filename, path } => {
                        match destination {
                            AttachmentDestination::Issue(issue_id) => {
                                sink.request_usecase(UsecaseRequest::StartAttachmentUpload {
                                    issue_id: *issue_id,
                                    filename,
                                    path,
                                });
                            }
                            AttachmentDestination::Draft(draft) => {
                                draft
                                    .borrow_mut()
                                    .attachments
                                    .push(DraftAttachment { filename, path });
                            }
                        }
                        self.popup_components.pop_back();
                    }
                    AttachmentInputPopupEventProcessResult::Canceled => {
                        self.popup_components.pop_back();
                    }
                    AttachmentInputPopupEventProcessResult::Handled => {}
                }
            }
            PopupComponent::SelectBox(popup_component) => {
                let Some(result) = popup_component.process_event(event) else {
                    return false;
                };
                match result {
                    SelectBoxPopupEventProcessResult::Entered
                    | SelectBoxPopupEventProcessResult::Quited => {
                        self.popup_components.pop_back();
                    }
                    SelectBoxPopupEventProcessResult::Handled => {}
                }
            }
            PopupComponent::SpentTimeInput(popup_component) => {
                let Some(result) = popup_component.process_event(event) else {
                    return false;
                };
                match result {
                    SpentTimeInputPopupEventProcessResult::Quited => {
                        self.popup_components.pop_back();
                    }
                    SpentTimeInputPopupEventProcessResult::OpenTimeEntityActivitiesPopup => {
                        let popup_component = Self::create_spent_time_input_popup_component(
                            dispatcher.clone(),
                            popup_component_rc.clone(),
                        );
                        self.popup_components.push_back(popup_component);
                    }
                    SpentTimeInputPopupEventProcessResult::Submited => {
                        // FIXME: Storeの更新
                        self.popup_components.pop_back();
                    }
                    SpentTimeInputPopupEventProcessResult::Handled => {}
                }
            }
            PopupComponent::DatePicker(popup_component) => {
                let Some(result) = popup_component.process_event(event) else {
                    return false;
                };
                match result {
                    DatePickerPopupEventProcessResult::Entered
                    | DatePickerPopupEventProcessResult::Canceled => {
                        self.popup_components.pop_back();
                    }
                    DatePickerPopupEventProcessResult::Handled => {}
                }
            }
            PopupComponent::NumberInput(popup_component) => {
                let Some(result) = popup_component.process_event(event) else {
                    return false;
                };
                match result {
                    NumberInputPopupEventProcessResult::Entered
                    | NumberInputPopupEventProcessResult::Canceled => {
                        self.popup_components.pop_back();
                    }
                    NumberInputPopupEventProcessResult::Handled => {}
                }
            }
            PopupComponent::IssueSelect(popup_component) => {
                let result = {
                    let dispatcher = dispatcher.borrow();
                    popup_component.process_event(event, dispatcher.store(), sink)
                };
                let Some(result) = result else {
                    return false;
                };
                match result {
                    IssueSelectPopupEventProcessResult::Selected { issue_id } => {
                        self.select_issue(issue_id, sink);
                    }
                    IssueSelectPopupEventProcessResult::CreateRequested { project_id } => {
                        self.popup_components.pop_back();
                        self.main_component = Some(MainComponent::NewIssue(
                            NewIssueComponent::new(project_id, None),
                        ));
                    }
                    IssueSelectPopupEventProcessResult::Quited => {
                        self.close_issue_select_popup();
                    }
                    IssueSelectPopupEventProcessResult::Handled => {}
                }
            }
            PopupComponent::IssuePropertyConflict {
                issue_id,
                component,
            } => {
                let Some(result) = component.process_event(event) else {
                    return false;
                };
                match result {
                    IssuePropertyConflictEventProcessResult::Canceled => {
                        sink.request_usecase(UsecaseRequest::CancelIssueUpload { id: *issue_id });
                        self.popup_components.pop_back();
                    }
                    IssuePropertyConflictEventProcessResult::Continued { diffs } => {
                        sink.request_usecase(UsecaseRequest::ContinueIssueUpload {
                            id: *issue_id,
                            selected_local_diffs: diffs,
                        });
                        self.popup_components.pop_back();
                    }
                    IssuePropertyConflictEventProcessResult::Handled => {}
                }
            }
            PopupComponent::RemoteJournalConflict {
                issue_id,
                journal_id,
                component,
            } => {
                let Some(result) = component.process_event(event) else {
                    return false;
                };
                match result {
                    RemoteJournalConflictEventProcessResult::Canceled => {
                        sink.request_usecase(UsecaseRequest::CancelRemoteJournalUpload {
                            issue_id: *issue_id,
                            journal_id: *journal_id,
                        });
                        self.popup_components.pop_back();
                    }
                    RemoteJournalConflictEventProcessResult::Continued { resolved_notes } => {
                        // FIXME: 競合情報を同期的に消さないため、直後のupdateで古いサーバーnotesのままpopupが
                        // 開き直し、続行中に届いた新しい競合でも更新されない。
                        sink.request_usecase(UsecaseRequest::ContinueRemoteJournalUpload {
                            issue_id: *issue_id,
                            journal_id: *journal_id,
                            resolved_notes,
                        });
                        self.popup_components.pop_back();
                    }
                    RemoteJournalConflictEventProcessResult::Handled => {}
                }
            }
        }
        true
    }

    /// NewIssueComponentへイベントを渡し、結果に応じてpopupを開く。
    /// NewIssueComponentがイベントを使った場合にtrueを返す。
    fn process_new_issue_event(&mut self, event: InputEvent, sink: &mut RequestSink) -> bool {
        let Some(MainComponent::NewIssue(new_issue_component)) = &mut self.main_component else {
            unreachable!("caller checked that NewIssueComponent exists");
        };
        let Some(result) = new_issue_component.process_event(&event) else {
            return false;
        };
        let creation_id = new_issue_component.creation_id();
        let draft = new_issue_component.draft();
        match result {
            NewIssueEventProcessResult::EditFieldRequested { field } => {
                let (context, initial_text) = match field {
                    DraftField::Subject => (
                        PendingEditorContext::NewIssueSubject { creation_id },
                        draft.borrow().subject.clone(),
                    ),
                    DraftField::Description => (
                        PendingEditorContext::NewIssueDescription { creation_id },
                        draft.borrow().description.clone(),
                    ),
                    DraftField::StartDate | DraftField::DueDate => {
                        let (selected_date, observer) = new_issue_component.date_popup(field);
                        self.popup_components.push_back(Rc::new(RefCell::new(
                            PopupComponent::DatePicker(DatePickerPopupComponent::new(
                                selected_date,
                                observer,
                            )),
                        )));
                        return true;
                    }
                    DraftField::EstimatedHours => {
                        let (estimated_hours, observer) =
                            new_issue_component.estimated_hours_popup();
                        self.popup_components.push_back(Rc::new(RefCell::new(
                            PopupComponent::NumberInput(NumberInputPopupComponent::new(
                                "予定工数",
                                estimated_hours,
                                observer,
                            )),
                        )));
                        return true;
                    }
                    field => {
                        let select_box = {
                            let dispatcher = self.dispatcher.borrow();
                            new_issue_component.select_box(dispatcher.store(), field)
                        };
                        if let Some(select_box) = select_box {
                            self.push_select_box_popup(
                                &select_box.items,
                                select_box.focused_index,
                                true,
                                select_box.observer,
                            );
                        }
                        return true;
                    }
                };
                self.start_editing(context, initial_text, sink);
            }
            NewIssueEventProcessResult::SubmitRequested => {
                let draft = draft.borrow().clone();
                sink.request_usecase(UsecaseRequest::CreateIssue { creation_id, draft });
            }
            NewIssueEventProcessResult::SubjectRequired => {
                self.toast_list.push_new_issue_subject_required();
            }
            NewIssueEventProcessResult::AddAttachmentRequested => {
                self.push_attachment_input_popup(AttachmentDestination::Draft(draft));
            }
            NewIssueEventProcessResult::OpenIssueSelectPopup => {
                self.open_issue_select_popup(None, sink);
            }
            NewIssueEventProcessResult::Handled => {}
        }
        true
    }

    /// IssueComponentへイベントを渡し、結果に応じてpopupの開閉や要求の発行を行う。
    /// IssueComponentがイベントを使った場合にtrueを返す。
    fn process_issue_event(
        &mut self,
        event: InputEvent,
        dispatcher: Rc<RefCell<Dispatcher>>,
        sink: &mut RequestSink,
    ) -> bool {
        let (result, issue_id) = {
            let app_dispatcher = self.dispatcher.clone();
            let issue_component = self
                .issue_component_mut()
                .expect("caller checked that IssueComponent exists");
            (
                issue_component.process_event(event, app_dispatcher),
                issue_component.issue_id(),
            )
        };
        let Some(result) = result else {
            return false;
        };
        match result {
            IssueEventProcessResult::FetchRequested { id } => {
                sink.request_usecase(UsecaseRequest::FetchIssue {
                    id,
                    with_parent: true,
                });
            }
            IssueEventProcessResult::UploadConfirmationRetryRequested { id } => {
                sink.request_usecase(UsecaseRequest::RetryIssueUploadConfirmation { id });
            }
            IssueEventProcessResult::OpenIssueSelectPopup => {
                self.open_issue_select_popup(Some(issue_id), sink);
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::EditIssueBodyRequested { id, body },
            ) => {
                let context = PendingEditorContext::IssueBody { id };
                if can_start_editing(&context, self.interaction_mode, dispatcher.borrow().store()) {
                    self.pending_editor_context = Some(context);
                    sink.request_editor(EditorRequest { initial_text: body });
                    self.interaction_mode = InteractionMode::Editing;
                }
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenIssueStatusPopup,
            ) => {
                let (items, focused_index) =
                    build_issue_status_options(dispatcher.borrow().store());
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    issue_status_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenTrackerPopup) => {
                let (items, focused_index) = {
                    let dispatcher = dispatcher.borrow();
                    let store = dispatcher.store();
                    let current = store.get_issue(issue_id).0.tracker_id();
                    build_tracker_options(store, Some(current))
                };
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    tracker_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            // FIXME: 子Issueを持つ親Issueの優先度・開始日・期日・進捗率は子から計算され、Redmineは更新を
            // エラーなしで無視する。親Issueでも編集できるため、ローカルでは変更済みに見えてしまう。
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenPriorityPopup) => {
                let (items, focused_index) = {
                    let dispatcher = dispatcher.borrow();
                    let store = dispatcher.store();
                    let current = store.get_issue(issue_id).0.priority_id();
                    build_priority_options(store, Some(current))
                };
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    priority_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenProjectPopup) => {
                let (items, focused_index) =
                    build_project_options(dispatcher.borrow().store(), issue_id);
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    project_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenAssignedToPopup) => {
                let (items, focused_index) = {
                    let dispatcher = dispatcher.borrow();
                    let store = dispatcher.store();
                    let current = store.get_issue(issue_id).0.assigned_to_id();
                    build_assigned_to_options(store, current)
                };
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    true,
                    assigned_to_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenTargetVersionPopup,
            ) => {
                let (items, focused_index) = {
                    let dispatcher = dispatcher.borrow();
                    let store = dispatcher.store();
                    let (issue, _) = store.get_issue(issue_id);
                    build_target_version_options(
                        store,
                        issue.project_id(),
                        issue.target_version_id(),
                    )
                };
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    true,
                    target_version_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenStartDatePopup) => {
                let selected_date = current_start_date(dispatcher.borrow().store(), issue_id);
                self.popup_components
                    .push_back(Rc::new(RefCell::new(PopupComponent::DatePicker(
                        DatePickerPopupComponent::new(
                            selected_date,
                            start_date_popup_observer(dispatcher.clone(), issue_id),
                        ),
                    ))));
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenDueDatePopup) => {
                let selected_date = current_due_date(dispatcher.borrow().store(), issue_id);
                self.popup_components
                    .push_back(Rc::new(RefCell::new(PopupComponent::DatePicker(
                        DatePickerPopupComponent::new(
                            selected_date,
                            due_date_popup_observer(dispatcher.clone(), issue_id),
                        ),
                    ))));
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenDoneRatioPopup) => {
                let current = dispatcher
                    .borrow()
                    .store()
                    .get_issue(issue_id)
                    .0
                    .done_ratio();
                let (items, focused_index) = build_done_ratio_options(Some(current));
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    false,
                    done_ratio_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::OpenCategoryPopup) => {
                let (items, focused_index) = {
                    let dispatcher = dispatcher.borrow();
                    let store = dispatcher.store();
                    let (issue, _) = store.get_issue(issue_id);
                    build_category_options(store, issue.project_id(), issue.category_id())
                };
                self.push_select_box_popup(
                    &items,
                    focused_index,
                    true,
                    category_popup_observer(dispatcher.clone(), issue_id),
                );
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenEstimatedHoursPopup,
            ) => {
                let estimated_hours =
                    current_estimated_hours(dispatcher.borrow().store(), issue_id);
                self.popup_components.push_back(Rc::new(RefCell::new(
                    PopupComponent::NumberInput(NumberInputPopupComponent::new(
                        "予定工数",
                        estimated_hours,
                        estimated_hours_popup_observer(dispatcher.clone(), issue_id),
                    )),
                )));
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenSpentTimeInputPopup,
            ) => {
                self.popup_components.push_back(Rc::new(RefCell::new(
                    PopupComponent::SpentTimeInput(SpentTimeInputPopupComponent::new(
                        dispatcher.borrow().store(),
                    )),
                )));
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::AddAttachmentRequested { issue_id },
            ) => {
                self.push_attachment_input_popup(AttachmentDestination::Issue(issue_id));
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::StartIssueUpload) => {
                sink.request_usecase(UsecaseRequest::StartIssueUpload { id: issue_id });
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::SaveRequested {
                issue_id,
                id,
            }) => {
                let is_edited = {
                    let dispatcher = dispatcher.borrow();
                    matches!(
                        dispatcher.store().get_remote_journal(issue_id, id).state,
                        RemoteJournalState::Edited { .. }
                    )
                };
                if is_edited {
                    sink.request_usecase(UsecaseRequest::StartRemoteJournalUpload {
                        issue_id,
                        journal_id: id,
                    });
                }
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::SaveLocalJournalRequested { issue_id },
            ) => {
                // Local Journal側が保存可能な状態でだけ要求を返すため、ここでは状態を再検査しない。
                sink.request_usecase(UsecaseRequest::StartLocalJournalUpload { issue_id });
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::SaveDeletedJournalRequested {
                    issue_id,
                    original_id,
                },
            ) => {
                // 退避したJournal側が投稿可能な状態でだけ要求を返すため、ここでは状態を再検査しない。
                sink.request_usecase(UsecaseRequest::StartDeletedJournalUpload {
                    issue_id,
                    original_id,
                });
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::DiscardDeletedJournalRequested {
                    issue_id,
                    original_id,
                },
            ) => {
                self.dispatcher.borrow_mut().dispatch(Action::Journal(
                    JournalAction::DiscardDeleted {
                        issue_id,
                        original_id,
                    },
                ));
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::EditDeletedJournalRequested {
                    issue_id,
                    original_id,
                    notes,
                },
            ) => {
                let context = PendingEditorContext::DeletedJournal {
                    issue_id,
                    original_id,
                };
                if can_start_editing(&context, self.interaction_mode, dispatcher.borrow().store()) {
                    self.pending_editor_context = Some(context);
                    sink.request_editor(EditorRequest {
                        initial_text: notes,
                    });
                    self.interaction_mode = InteractionMode::Editing;
                }
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::EditJournalRequested {
                    issue_id,
                    id,
                    notes,
                },
            ) => {
                let context = PendingEditorContext::RemoteJournal {
                    issue_id,
                    journal_id: id,
                };
                if can_start_editing(&context, self.interaction_mode, dispatcher.borrow().store()) {
                    self.pending_editor_context = Some(context);
                    sink.request_editor(EditorRequest {
                        initial_text: notes,
                    });
                    self.interaction_mode = InteractionMode::Editing;
                }
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::EditLocalJournalRequested { issue_id, notes },
            ) => {
                let context = PendingEditorContext::LocalJournal { issue_id };
                if can_start_editing(&context, self.interaction_mode, dispatcher.borrow().store()) {
                    self.pending_editor_context = Some(context);
                    sink.request_editor(EditorRequest {
                        initial_text: notes,
                    });
                    self.interaction_mode = InteractionMode::Editing;
                }
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::CreateLocalJournalRequested { issue_id },
            ) => {
                if self.interaction_mode == InteractionMode::Application {
                    self.dispatcher
                        .borrow_mut()
                        .dispatch(Action::Journal(JournalAction::CreateLocal { issue_id }));
                    self.pending_editor_context =
                        Some(PendingEditorContext::LocalJournal { issue_id });
                    sink.request_editor(EditorRequest {
                        initial_text: String::new(),
                    });
                    self.interaction_mode = InteractionMode::Editing;
                }
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::OpenIssueRequested { id },
            ) => {
                self.open_issue(id, sink);
            }
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::CreateChildRequested { parent_id },
            ) => {
                // 子は親と同じプロジェクトに作る。
                let project_id = dispatcher
                    .borrow()
                    .store()
                    .get_issue(parent_id)
                    .0
                    .project_id();
                self.main_component = Some(MainComponent::NewIssue(NewIssueComponent::new(
                    project_id,
                    Some(parent_id),
                )));
            }
            // 子Componentがキーを解釈済みで、App側に要求はない。
            IssueEventProcessResult::Detail(
                IssueDetailEventProcessResult::ComputedPropertyNotEditable { issue_id, property },
            ) => {
                self.toast_list
                    .push_computed_property_not_editable(issue_id, property);
            }
            IssueEventProcessResult::Detail(IssueDetailEventProcessResult::Handled) => {}
        }
        true
    }

    fn issue_component(&self) -> Option<&IssueComponent> {
        match &self.main_component {
            Some(MainComponent::Issue(issue_component)) => Some(issue_component),
            Some(MainComponent::NewIssue(_)) | None => None,
        }
    }

    fn issue_component_mut(&mut self) -> Option<&mut IssueComponent> {
        match &mut self.main_component {
            Some(MainComponent::Issue(issue_component)) => Some(issue_component),
            Some(MainComponent::NewIssue(_)) | None => None,
        }
    }

    /// editorの起動を要求できる状態なら、文脈を保持してeditorを起動する。
    fn start_editing(
        &mut self,
        context: PendingEditorContext,
        initial_text: String,
        sink: &mut RequestSink,
    ) {
        if can_start_editing(
            &context,
            self.interaction_mode,
            self.dispatcher.borrow().store(),
        ) {
            self.pending_editor_context = Some(context);
            sink.request_editor(EditorRequest { initial_text });
            self.interaction_mode = InteractionMode::Editing;
        }
    }

    fn select_issue(&mut self, issue_id: IssueId, sink: &mut RequestSink) {
        self.popup_components.pop_back();
        self.open_issue(issue_id, sink);
    }

    /// Issueを開き、Issue自身と、分かっていれば親の取得を要求する。起動するかは`fetch_issue`が判定する。
    ///
    /// 取得済みのIssueは取得の完了が起きず、完了の後続要求で親を取得できないため、
    /// Storeにある親IDで要求する。
    // FIXME: 親の取得(`with_parent: false`)が終わる前にその親を開くと、取得中なので
    // `FetchIssue { with_parent: true }`は起動されず、Storeに親IDもないため、さらに上の親を取得しない。
    // 起動済みの取得に「親も取得する」希望を後から伝える手段がない。
    fn open_issue(&mut self, issue_id: IssueId, sink: &mut RequestSink) {
        let parent_id = {
            let dispatcher = self.dispatcher.borrow();
            let store = dispatcher.store();
            self.main_component = Some(MainComponent::Issue(IssueComponent::new(store, issue_id)));
            store
                .try_get_issue_state(issue_id)
                .and_then(|_| store.get_issue(issue_id).0.parent_id())
        };
        sink.request_usecase(UsecaseRequest::FetchIssue {
            id: issue_id,
            with_parent: true,
        });
        if let Some(parent_id) = parent_id {
            sink.request_usecase(UsecaseRequest::FetchIssue {
                id: parent_id,
                with_parent: false,
            });
        }
    }

    fn open_issue_select_popup(
        &mut self,
        focused_issue_id: Option<IssueId>,
        sink: &mut RequestSink,
    ) {
        let popup = {
            let dispatcher = self.dispatcher.borrow();
            IssueSelectPopupComponent::new(dispatcher.store(), focused_issue_id, sink)
        };
        self.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::IssueSelect(popup))));
    }

    fn close_issue_select_popup(&mut self) {
        self.popup_components.pop_back();
    }

    fn push_attachment_input_popup(&mut self, destination: AttachmentDestination) {
        self.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::AttachmentInput {
                destination,
                component: AttachmentInputPopupComponent::new(),
            })));
    }

    /// SelectBoxPopupComponentを構築し、popup stackへ積む。
    fn push_select_box_popup(
        &mut self,
        items: &[(u32, String)],
        focused_index: usize,
        include_none: bool,
        observer: Box<dyn FnMut(Option<u32>) + 'a>,
    ) {
        self.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::SelectBox(
                SelectBoxPopupComponent::new(items, focused_index, include_none, observer),
            ))));
    }

    /// どのComponentも使わなかったqだけをアプリ終了としてfalseを返す。
    pub fn handle_key_event(
        &mut self,
        event: InputEvent,
        dispatcher: Rc<RefCell<Dispatcher>>,
        sink: &mut RequestSink,
    ) -> bool {
        let is_q = matches!(
            event,
            InputEvent::Key(key) if key.code == KeyCode::Char('q')
        );
        let handled = self.process_event(event, dispatcher, sink);
        !is_q || handled
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store, area: Rect) {
        self.open_issue_property_conflict_popup_if_needed(store);
        self.open_remote_journal_conflict_popups_if_needed(store);

        match &mut self.main_component {
            Some(MainComponent::Issue(issue_component)) => {
                issue_component.update(dispatcher, store, (area.width, area.height));
            }
            Some(MainComponent::NewIssue(new_issue_component)) => {
                new_issue_component.update(area.height);
            }
            None => {}
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
            if let PopupComponent::RemoteJournalConflict { component, .. } =
                &mut *popup_component.borrow_mut()
            {
                component.update(area);
            }
        }
    }

    fn open_issue_property_conflict_popup_if_needed(&mut self, store: &Store) {
        if let Some(issue_component) = self.issue_component()
            && let Some((server_issue, conflicts)) =
                store.try_get_issue_upload_conflict(issue_component.issue_id())
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
    }

    fn open_remote_journal_conflict_popups_if_needed(&mut self, store: &Store) {
        if let Some(issue_component) = self.issue_component() {
            let issue_id = issue_component.issue_id();
            for entry in store.get_remote_journals(issue_id) {
                let journal_id = entry.journal.id;
                let Some((diff, conflict)) =
                    store.try_get_remote_journal_upload_conflict(issue_id, journal_id)
                else {
                    continue;
                };
                if !self.popup_components.iter().any(|popup| {
                    matches!(
                        &*popup.borrow(),
                        PopupComponent::RemoteJournalConflict {
                            issue_id: popup_issue_id,
                            journal_id: popup_journal_id,
                            ..
                        }
                            if *popup_issue_id == issue_id
                                && *popup_journal_id == journal_id
                    )
                }) {
                    let diff = diff.clone();
                    let conflict = conflict.clone();
                    self.popup_components.push_back(Rc::new(RefCell::new(
                        PopupComponent::RemoteJournalConflict {
                            issue_id,
                            journal_id,
                            component: RemoteJournalConflictComponent::new(
                                diff,
                                conflict.server_notes.clone(),
                            ),
                        },
                    )));
                }
            }
        }
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, area: Rect) {
        match &self.main_component {
            Some(MainComponent::Issue(issue_component)) => {
                issue_component.render(store, frame, area)
            }
            Some(MainComponent::NewIssue(new_issue_component)) => {
                new_issue_component.render(store, frame, area)
            }
            None => {}
        }
        self.render_popup_component(frame, area, store);
        // popupより後に描画して最前面へ重ねる。toastはfocusを持たずcursor位置も変えない。
        frame.render_widget(self.toast_list.create_widget(), area);
        if let Some(cursor_position) = self.cursor_position(store, area) {
            match self.cursor_rendering {
                CursorRendering::Terminal => frame.set_cursor_position(cursor_position),
                CursorRendering::Emulated => {
                    // 元のstyleが既にreverseでもカーソル位置が判別できるよう、反転を打ち消す。
                    if let Some(cell) = frame.buffer_mut().cell_mut(cursor_position) {
                        cell.modifier.toggle(Modifier::REVERSED);
                    }
                }
            }
        }
    }

    pub fn interaction_mode(&self) -> InteractionMode {
        self.interaction_mode
    }

    /// 表示経過時間を進める。
    pub fn tick(&mut self, elapsed: chrono::Duration) {
        self.toast_list.tick(elapsed);
    }

    pub fn handle_usecase_events(&mut self, events: Vec<UsecaseEvent>, store: &Store) {
        for event in events {
            match event {
                // 作成中に別のIssueへ移っていれば、作成した下書きはもう表示していない。
                UsecaseEvent::IssueCreated { creation_id, id } => {
                    if self.new_issue_component_mut(creation_id).is_some() {
                        self.main_component =
                            Some(MainComponent::Issue(IssueComponent::new(store, id)));
                    }
                }
                UsecaseEvent::Failure(failure) => {
                    if let UsecaseFailure::IssueCreate { creation_id, .. } = &failure
                        && let Some(new_issue_component) =
                            self.new_issue_component_mut(*creation_id)
                    {
                        new_issue_component.finish_submitting();
                    }
                    self.toast_list.push_failure(&failure);
                }
            }
        }
    }

    /// 表示中の新規作成画面が`creation_id`の下書きなら返す。
    fn new_issue_component_mut(
        &mut self,
        creation_id: IssueCreationId,
    ) -> Option<&mut NewIssueComponent> {
        match &mut self.main_component {
            Some(MainComponent::NewIssue(new_issue_component))
                if new_issue_component.creation_id() == creation_id =>
            {
                Some(new_issue_component)
            }
            _ => None,
        }
    }

    pub fn notify_image_display_unavailable(&mut self, reason: &str) {
        self.toast_list.push_image_display_unavailable(reason);
    }

    /// editorの失敗を通知し、editor contextを解放する。
    pub fn handle_editor_failure(&mut self, error: &dyn std::fmt::Display) {
        self.handle_editor_response(EditorOutcome::Failed);
        self.toast_list.push_editor_failure(error);
    }

    pub fn handle_editor_response(&mut self, outcome: EditorOutcome) {
        // CancelledやFailedを含むすべての完了経路で通常入力へ戻し、editor contextを解放する。
        self.interaction_mode = InteractionMode::Application;
        let context = self.pending_editor_context.take();
        let EditorOutcome::Submitted { edited_text } = outcome else {
            return;
        };
        match context {
            Some(PendingEditorContext::IssueBody { id }) => {
                self.dispatcher
                    .borrow_mut()
                    .dispatch(IssueAction::UpdateDescription {
                        id,
                        body: edited_text,
                    });
            }
            Some(PendingEditorContext::RemoteJournal {
                issue_id,
                journal_id,
            }) => {
                self.dispatcher.borrow_mut().dispatch(Action::Journal(
                    JournalAction::EditRemoteNotes {
                        issue_id,
                        journal_id,
                        notes: edited_text,
                    },
                ));
            }
            Some(PendingEditorContext::LocalJournal { issue_id }) => {
                self.dispatcher.borrow_mut().dispatch(Action::Journal(
                    JournalAction::EditLocalNotes {
                        issue_id,
                        notes: edited_text,
                    },
                ));
            }
            Some(PendingEditorContext::DeletedJournal {
                issue_id,
                original_id,
            }) => {
                self.dispatcher.borrow_mut().dispatch(Action::Journal(
                    JournalAction::EditDeletedNotes {
                        issue_id,
                        original_id,
                        notes: edited_text,
                    },
                ));
            }
            Some(
                context @ (PendingEditorContext::NewIssueSubject { creation_id }
                | PendingEditorContext::NewIssueDescription { creation_id }),
            ) => {
                // editor中は入力を受け付けないため、下書きは編集を始めたときのまま表示中である。
                let Some(MainComponent::NewIssue(new_issue_component)) = &self.main_component
                else {
                    return;
                };
                if new_issue_component.creation_id() != creation_id {
                    return;
                }
                let draft = new_issue_component.draft();
                let mut draft = draft.borrow_mut();
                match context {
                    PendingEditorContext::NewIssueSubject { .. } => {
                        draft.subject = single_line(&edited_text);
                    }
                    _ => draft.description = edited_text,
                }
            }
            None => {}
        }
    }

    fn render_popup_component(&self, frame: &mut Frame, area: Rect, store: &Store) {
        for popup_component in &self.popup_components {
            let popup_component = popup_component.clone();
            match &mut *(popup_component.borrow_mut()) {
                PopupComponent::AttachmentInput { component, .. } => {
                    let widget = component.create_widget();
                    frame.render_widget(widget, area);
                }
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
                PopupComponent::NumberInput(popup_component) => {
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
                PopupComponent::RemoteJournalConflict { component, .. } => {
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
                // ratatui_textarea::TextAreaが表示するカーソルをそのまま使用する
                PopupComponent::AttachmentInput { .. } => None,
                PopupComponent::SelectBox(_) => None,
                PopupComponent::SpentTimeInput(popup_component) => {
                    popup_component.cursor_position(area)
                }
                PopupComponent::DatePicker(_) => None,
                // ratatui_textarea::TextAreaが表示するカーソルをそのまま使用する
                PopupComponent::NumberInput(_) => None,
                PopupComponent::IssueSelect(_) => None,
                PopupComponent::IssuePropertyConflict { component, .. } => {
                    component.cursor_position(area)
                }
                PopupComponent::RemoteJournalConflict { component, .. } => {
                    component.cursor_position(area)
                }
            }
        } else if let Some(issue_component) = self.issue_component() {
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
    use std::num::NonZeroUsize;

    use crate::entities::{Issue, ProjectIssuesPage};
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};
    use crate::stores::{JournalAction, ProjectIssuesAction};
    use crate::vos::ProjectId;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    #[test]
    fn handle_editor_failure_returns_to_application_mode() {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let mut app = AppComponent::new(
            dispatcher.clone(),
            None,
            CursorRendering::Terminal,
            &mut RequestSink::default(),
        );
        app.interaction_mode = InteractionMode::Editing;

        app.handle_editor_failure(&"boom");

        assert_eq!(app.interaction_mode(), InteractionMode::Application);
    }

    fn assert_no_requests(sink: &mut RequestSink) {
        assert_eq!(sink.take_usecases(), []);
        assert!(sink.take_editor().is_none());
    }

    fn key_event(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::none()))
    }

    fn ctrl_s_event() -> InputEvent {
        InputEvent::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::control()))
    }

    #[test]
    fn can_start_editing_issue_body_unless_issue_is_uploading() {
        let id = IssueId::new(3);
        let context = PendingEditorContext::IssueBody { id };
        let mut store = Store::new();
        crate::test_support::load_issue(&mut store, crate::test_support::sample_parent_issue());

        assert!(can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));

        store.consume_action(
            IssueAction::UpdateDescription {
                id,
                body: "updated body".to_string(),
            }
            .into(),
        );
        store.consume_action(IssueAction::StartUpload { id }.into());

        assert!(!can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));
    }

    #[test]
    fn submitted_empty_issue_body_is_applied_and_releases_editor_context() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.pending_editor_context = Some(PendingEditorContext::IssueBody { id: 3.into() });

        app.handle_editor_response(EditorOutcome::Submitted {
            edited_text: String::new(),
        });

        assert!(app.pending_editor_context.is_none());
        assert_eq!(app.interaction_mode(), InteractionMode::Application);
        dispatcher.borrow_mut().consume_action();
        assert_eq!(dispatcher.borrow().store().get_issue(3).0.description(), "");
    }

    #[test]
    fn cancelled_or_failed_editor_releases_context_without_dispatching_for_each_context() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        for failed in [false, true] {
            for context in [
                PendingEditorContext::IssueBody { id: 3.into() },
                PendingEditorContext::RemoteJournal {
                    issue_id: 3.into(),
                    journal_id: 1.into(),
                },
                PendingEditorContext::LocalJournal { issue_id: 3.into() },
            ] {
                app.pending_editor_context = Some(context);
                app.interaction_mode = InteractionMode::Editing;
                let outcome = if failed {
                    EditorOutcome::Failed
                } else {
                    EditorOutcome::Cancelled
                };
                app.handle_editor_response(outcome);
                assert!(app.pending_editor_context.is_none());
                assert_eq!(app.interaction_mode(), InteractionMode::Application);
            }
        }
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn can_start_editing_remote_journal_unless_it_is_uploading() {
        let issue_id = IssueId::new(3);
        let journal_id = JournalId::new(1);
        let context = PendingEditorContext::RemoteJournal {
            issue_id,
            journal_id,
        };
        let mut store = Store::new();
        let mut issue = crate::test_support::sample_parent_issue();
        issue.journals = vec![crate::test_support::sample_parent_issue_journals().remove(0)];
        crate::test_support::load_issue(&mut store, issue);

        assert!(can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));

        store.consume_action(
            JournalAction::EditRemoteNotes {
                issue_id,
                journal_id,
                notes: "edited notes".to_string(),
            }
            .into(),
        );

        assert!(can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));

        store.consume_action(
            JournalAction::StartRemoteUpload {
                issue_id,
                journal_id,
            }
            .into(),
        );

        assert!(!can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));
    }

    #[test]
    fn can_start_editing_local_journal_only_when_it_is_local_only() {
        let issue_id = IssueId::new(3);
        let context = PendingEditorContext::LocalJournal { issue_id };
        let mut store = Store::new();
        crate::test_support::load_issue(&mut store, crate::test_support::sample_parent_issue());

        assert!(!can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));

        store.consume_action(JournalAction::CreateLocal { issue_id }.into());

        assert!(can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));

        store.consume_action(JournalAction::StartLocalUpload { issue_id }.into());

        assert!(!can_start_editing(
            &context,
            InteractionMode::Application,
            &store
        ));
    }

    fn loaded_dispatcher() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_sample_masters(&mut dispatcher_ref);
            for action in
                crate::test_support::fetch_issue_actions(crate::test_support::sample_parent_issue())
            {
                dispatcher_ref.dispatch(action);
            }
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

    fn dispatcher_with_selectable_issues() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_sample_masters(&mut dispatcher_ref);
            for issue in [
                crate::test_support::sample_open_child_issue(),
                crate::test_support::sample_parent_issue(),
            ] {
                for action in crate::test_support::fetch_issue_actions(issue) {
                    dispatcher_ref.dispatch(action);
                }
            }
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
            issues: [1_u32, 3_u32, 42_u32]
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
        sink: &mut RequestSink,
    ) {
        let [UsecaseRequest::FetchProjectIssuesPage { project_id, page }] =
            sink.take_usecases()[..]
        else {
            panic!("expected the popup's initial project page request")
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

    fn loaded_dispatcher_with_journals() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_sample_masters(&mut dispatcher_ref);
            for action in crate::test_support::fetch_sample_parent_issue_actions(
                crate::test_support::sample_parent_issue_journals(),
            ) {
                dispatcher_ref.dispatch(action);
            }
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
        }
        dispatcher
    }

    /// Property(2カラム時の左カラム8行) -> Body -> ChildrenList -> JournalsList(先頭Journalのdetail)
    /// の順にフォーカスを送り、JournalsList内の1件目JournalのNotes位置に到達させる
    fn focus_first_journal_notes(app: &mut AppComponent<'_>, dispatcher: Rc<RefCell<Dispatcher>>) {
        let mut sink = RequestSink::default();
        for _ in 0..48 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone(), &mut sink);
            app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        }
    }

    #[test]
    fn new_with_loaded_initial_issue_creates_issue_component() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let app = AppComponent::new(
            dispatcher,
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );

        assert_eq!(app.issue_component().unwrap().issue_id(), IssueId::new(3));
        assert_eq!(app.interaction_mode(), InteractionMode::Application);
        assert_eq!(
            sink.take_usecases(),
            [UsecaseRequest::FetchIssue {
                id: IssueId::new(3),
                with_parent: true,
            }]
        );
    }

    #[test]
    fn process_event_returns_true_when_issue_detail_moves_focus() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        assert!(app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone(), &mut sink));
    }

    #[test]
    fn editing_mode_ignores_input_without_opening_popup_or_requesting() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.interaction_mode = InteractionMode::Editing;
        sink.take_usecases();

        app.process_event(key_event(KeyCode::Char('y')), dispatcher.clone(), &mut sink);

        assert!(app.popup_components.is_empty());
        assert_no_requests(&mut sink);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn new_with_unknown_initial_issue_requests_it_with_its_parent() {
        let mut sink = RequestSink::default();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        let app = AppComponent::new(
            dispatcher,
            Some(42.into()),
            CursorRendering::Terminal,
            &mut sink,
        );

        assert_eq!(app.issue_component().unwrap().issue_id(), IssueId::new(42));
        assert_eq!(
            sink.take_usecases(),
            [UsecaseRequest::FetchIssue {
                id: IssueId::new(42),
                with_parent: true,
            }]
        );
    }

    /// 子のIssue 1(親はIssue 3)だけを取得済みにする。
    fn dispatcher_with_loaded_child_only() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        crate::test_support::dispatch_loaded_issue(
            &mut dispatcher.borrow_mut(),
            crate::test_support::sample_open_child_issue(),
        );
        dispatcher
    }

    #[test]
    fn opening_a_fetched_issue_also_requests_its_parent() {
        let mut sink = RequestSink::default();
        let dispatcher = dispatcher_with_loaded_child_only();
        let _app = AppComponent::new(
            dispatcher.clone(),
            Some(1.into()),
            CursorRendering::Terminal,
            &mut sink,
        );

        assert_eq!(
            sink.take_usecases(),
            [
                UsecaseRequest::FetchIssue {
                    id: IssueId::new(1),
                    with_parent: true,
                },
                UsecaseRequest::FetchIssue {
                    id: IssueId::new(3),
                    with_parent: false,
                },
            ]
        );
    }

    #[test]
    fn process_event_without_initial_issue_ignores_issue_detail_keys() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            None,
            CursorRendering::Terminal,
            &mut sink,
        );
        app.process_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);
        sink.take_usecases();

        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('e'),
            KeyCode::Char('u'),
        ] {
            app.process_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        assert!(app.issue_component().is_none());
        assert!(app.popup_components.is_empty());
        assert_no_requests(&mut sink);
    }

    #[test]
    fn update_opens_issue_property_conflict_popup_only_once() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher_with_edited_issue();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        let conflicts = dispatcher
            .borrow()
            .store()
            .get_issue_property_diffs(IssueId::new(3))
            .to_vec();
        let server_issue = crate::test_support::sample_parent_issue();
        {
            let mut dispatcher = dispatcher.borrow_mut();
            dispatcher.dispatch(IssueAction::StartUpload { id: 3.into() });
            dispatcher.dispatch(IssueAction::UploadConflictsDetected {
                server_issue,
                conflicts,
                children: vec![],
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
    fn e_and_ctrl_s_on_uploading_local_journal_notes_request_nothing() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        for action in [
            JournalAction::CreateLocal {
                issue_id: IssueId::new(3),
            },
            JournalAction::EditLocalNotes {
                issue_id: IssueId::new(3),
                notes: "local notes".to_string(),
            },
            JournalAction::StartLocalUpload {
                issue_id: IssueId::new(3),
            },
        ] {
            dispatcher.borrow_mut().dispatch(Action::Journal(action));
            dispatcher.borrow_mut().consume_action();
        }

        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        sink.take_usecases();
        for _ in 0..100 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone(), &mut sink);
        }
        app.process_event(key_event(KeyCode::Char('k')), dispatcher.clone(), &mut sink);
        app.process_event(key_event(KeyCode::Char('e')), dispatcher.clone(), &mut sink);

        assert_no_requests(&mut sink);

        app.process_event(ctrl_s_event(), dispatcher.clone(), &mut sink);

        assert_no_requests(&mut sink);
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn ctrl_s_on_synced_journal_notes_requests_nothing() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher_with_journals();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        sink.take_usecases();

        focus_first_journal_notes(&mut app, dispatcher.clone());
        app.process_event(ctrl_s_event(), dispatcher.clone(), &mut sink);

        assert_no_requests(&mut sink);
    }

    /// updateでキャッシュしたプレビューがrenderで実際に描画されることを確認する
    #[test]
    fn startup_issue_select_popup_requests_its_initial_page_once() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let _app = AppComponent::new(dispatcher, None, CursorRendering::Terminal, &mut sink);

        assert_eq!(
            sink.take_usecases(),
            [UsecaseRequest::FetchProjectIssuesPage {
                project_id: ProjectId::new(1),
                page: NonZeroUsize::MIN,
            }]
        );
    }

    #[test]
    fn startup_without_initial_issue_selects_first_project_even_when_issue_store_has_other_projects()
     {
        let mut sink = RequestSink::default();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            crate::test_support::dispatch_sample_masters(&mut dispatcher_ref);
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
            for issue in [lower_id_issue, higher_id_issue] {
                for action in crate::test_support::fetch_issue_actions(issue) {
                    dispatcher_ref.dispatch(action);
                }
            }
            while dispatcher_ref.consume_actinos_len() > 0 {
                dispatcher_ref.consume_action();
            }
        }

        let _app = AppComponent::new(dispatcher, None, CursorRendering::Terminal, &mut sink);

        assert_eq!(
            sink.take_usecases(),
            [UsecaseRequest::FetchProjectIssuesPage {
                project_id: ProjectId::new(1),
                page: NonZeroUsize::MIN,
            }]
        );
    }

    #[test]
    fn startup_issue_select_popup_exposes_fetch_without_dispatching_project_issues_action() {
        let mut sink = RequestSink::default();
        let dispatcher = dispatcher_with_selectable_issues();
        let _app = AppComponent::new(
            dispatcher.clone(),
            None,
            CursorRendering::Terminal,
            &mut sink,
        );

        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
        assert_eq!(
            sink.take_usecases(),
            [UsecaseRequest::FetchProjectIssuesPage {
                project_id: ProjectId::new(1),
                page: NonZeroUsize::MIN,
            }]
        );
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    fn new_issue_component<'b>(app: &'b AppComponent<'_>) -> Option<&'b NewIssueComponent> {
        match &app.main_component {
            Some(MainComponent::NewIssue(new_issue_component)) => Some(new_issue_component),
            _ => None,
        }
    }

    #[test]
    fn n_on_issue_select_popup_opens_a_draft_in_the_focused_project() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            None,
            CursorRendering::Terminal,
            &mut sink,
        );
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), &mut sink);

        app.process_event(key_event(KeyCode::Char('n')), dispatcher.clone(), &mut sink);

        assert!(app.popup_components.is_empty());
        let draft = new_issue_component(&app)
            .expect("the draft should replace the main component")
            .draft();
        assert_eq!(draft.borrow().project_id, ProjectId::new(1));
        assert_eq!(draft.borrow().parent_issue_id, None);
        assert_no_requests(&mut sink);
    }

    fn app_with_draft(
        dispatcher: Rc<RefCell<Dispatcher>>,
        sink: &mut RequestSink,
    ) -> AppComponent<'static> {
        let mut app = AppComponent::new(dispatcher.clone(), None, CursorRendering::Terminal, sink);
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), sink);
        app.process_event(key_event(KeyCode::Char('n')), dispatcher, sink);
        app
    }

    #[test]
    fn editor_edits_the_draft_subject_as_a_single_line() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);

        app.process_event(key_event(KeyCode::Enter), dispatcher.clone(), &mut sink);
        assert_eq!(
            sink.take_editor().map(|request| request.initial_text),
            Some(String::new())
        );
        assert_eq!(app.interaction_mode(), InteractionMode::Editing);
        app.handle_editor_response(EditorOutcome::Submitted {
            edited_text: "  ログイン\n画面の  不具合\n".to_string(),
        });

        let draft = new_issue_component(&app).unwrap().draft();
        assert_eq!(draft.borrow().subject, "ログイン 画面の 不具合");
        assert_eq!(app.interaction_mode(), InteractionMode::Application);
    }

    #[test]
    fn editor_edits_the_draft_description_from_its_current_text() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);
        new_issue_component(&app)
            .unwrap()
            .draft()
            .borrow_mut()
            .description = "before".to_string();

        for code in [KeyCode::Char('j'), KeyCode::Enter] {
            app.process_event(key_event(code), dispatcher.clone(), &mut sink);
        }
        assert_eq!(
            sink.take_editor().map(|request| request.initial_text),
            Some("before".to_string())
        );
        app.handle_editor_response(EditorOutcome::Submitted {
            edited_text: "1行目\n2行目\n".to_string(),
        });

        let draft = new_issue_component(&app).unwrap().draft();
        assert_eq!(draft.borrow().description, "1行目\n2行目\n");
    }

    #[test]
    fn select_box_popup_sets_the_draft_tracker() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);

        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('j'),
            KeyCode::Enter,
            KeyCode::Char('j'),
            KeyCode::Enter,
        ] {
            app.process_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        assert!(app.popup_components.is_empty());
        let draft = new_issue_component(&app).unwrap().draft();
        assert_eq!(
            draft.borrow().tracker_id,
            Some(crate::vos::TrackerId::new(2))
        );
        assert_no_requests(&mut sink);
    }

    #[test]
    fn dates_and_estimated_hours_of_the_draft_open_their_input_popups() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);
        // 開始日・期日・予定工数は、題名から数えて7・8・10番目の項目である。
        for _ in 0..7 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone(), &mut sink);
        }

        app.process_event(key_event(KeyCode::Enter), dispatcher.clone(), &mut sink);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::DatePicker(_)
        ));
        app.popup_components.clear();
        for code in [KeyCode::Char('j'), KeyCode::Char('j'), KeyCode::Char('j')] {
            app.process_event(key_event(code), dispatcher.clone(), &mut sink);
        }
        app.process_event(key_event(KeyCode::Enter), dispatcher.clone(), &mut sink);

        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::NumberInput(_)
        ));
    }

    #[test]
    fn attachment_input_on_a_draft_adds_the_file_to_the_draft_without_uploading() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);
        for _ in 0..20 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone(), &mut sink);
        }

        for code in [KeyCode::Enter]
            .into_iter()
            .chain("a.png".chars().map(KeyCode::Char))
            .chain([KeyCode::Tab])
            .chain("/tmp/a.png".chars().map(KeyCode::Char))
            .chain([KeyCode::Enter])
        {
            app.process_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        assert!(app.popup_components.is_empty());
        let draft = new_issue_component(&app).unwrap().draft();
        assert_eq!(
            draft.borrow().attachments,
            vec![DraftAttachment {
                filename: "a.png".to_string(),
                path: std::path::PathBuf::from("/tmp/a.png"),
            }]
        );
        assert_no_requests(&mut sink);
    }

    fn ctrl_key(code: KeyCode) -> InputEvent {
        InputEvent::Key(KeyEvent::new(code, KeyModifiers::control()))
    }

    /// 題名を入れた下書きの作成を要求し、その`creation_id`を返す。
    fn submit_draft(
        app: &mut AppComponent<'_>,
        dispatcher: Rc<RefCell<Dispatcher>>,
        sink: &mut RequestSink,
    ) -> IssueCreationId {
        new_issue_component(app)
            .unwrap()
            .draft()
            .borrow_mut()
            .subject = "新しいIssue".to_string();
        app.process_event(ctrl_key(KeyCode::Char('s')), dispatcher, sink);
        match &sink.take_usecases()[..] {
            [UsecaseRequest::CreateIssue { creation_id, draft }] => {
                assert_eq!(draft.subject, "新しいIssue");
                *creation_id
            }
            requests => panic!("expected one CreateIssue request, got {requests:?}"),
        }
    }

    #[test]
    fn ctrl_s_on_a_draft_without_subject_tells_that_the_subject_is_required() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);

        app.process_event(ctrl_key(KeyCode::Char('s')), dispatcher.clone(), &mut sink);

        assert_no_requests(&mut sink);
        assert_eq!(
            app.toast_list.create_widget().messages,
            ["題名を入力してから作成してください"]
        );
    }

    #[test]
    fn created_event_replaces_the_submitted_draft_with_the_created_issue() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);
        let creation_id = submit_draft(&mut app, dispatcher.clone(), &mut sink);

        app.handle_usecase_events(
            vec![UsecaseEvent::IssueCreated {
                creation_id,
                id: IssueId::new(99),
            }],
            dispatcher.borrow().store(),
        );

        assert_eq!(app.issue_component().unwrap().issue_id(), IssueId::new(99));
    }

    #[test]
    fn created_event_of_a_discarded_draft_is_ignored() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);
        let creation_id = submit_draft(&mut app, dispatcher.clone(), &mut sink);
        app.process_event(key_event(KeyCode::Char('y')), dispatcher.clone(), &mut sink);
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), &mut sink);
        app.process_event(key_event(KeyCode::Char('n')), dispatcher.clone(), &mut sink);

        app.handle_usecase_events(
            vec![UsecaseEvent::IssueCreated {
                creation_id,
                id: IssueId::new(99),
            }],
            dispatcher.borrow().store(),
        );

        assert!(new_issue_component(&app).is_some());
    }

    #[test]
    fn create_failure_lets_the_submitted_draft_be_edited_again_and_tells_it() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = app_with_draft(dispatcher.clone(), &mut sink);
        let creation_id = submit_draft(&mut app, dispatcher.clone(), &mut sink);

        app.handle_usecase_events(
            vec![UsecaseEvent::Failure(UsecaseFailure::IssueCreate {
                creation_id,
                cause: crate::usecases::IssueCreateFailure::Create {
                    reason: "offline".to_string(),
                },
            })],
            dispatcher.borrow().store(),
        );

        assert_eq!(
            app.toast_list.create_widget().messages,
            ["Issueの作成に失敗しました: offline"]
        );
        app.process_event(ctrl_key(KeyCode::Char('s')), dispatcher.clone(), &mut sink);
        assert!(matches!(
            &sink.take_usecases()[..],
            [UsecaseRequest::CreateIssue { .. }]
        ));
    }

    #[test]
    fn add_child_row_opens_a_draft_of_a_child_in_the_parent_project() {
        let mut sink = RequestSink::default();
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        crate::test_support::dispatch_loaded_issue(
            &mut dispatcher.borrow_mut(),
            crate::test_support::sample_closed_child_issue(),
        );
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(2.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        // 子もJournalもないIssue 2では、12回のjで子一覧の追加行に届く(IssueDetailComponentのテストと同じ)。
        for _ in 0..12 {
            app.process_event(key_event(KeyCode::Char('j')), dispatcher.clone(), &mut sink);
            app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        }
        sink.take_usecases();

        app.process_event(key_event(KeyCode::Enter), dispatcher.clone(), &mut sink);

        let draft = new_issue_component(&app)
            .expect("the child draft should replace the issue")
            .draft();
        let parent_project_id = dispatcher.borrow().store().get_issue(2).0.project_id();
        assert_eq!(draft.borrow().project_id, parent_project_id);
        assert_eq!(draft.borrow().parent_issue_id, Some(IssueId::new(2)));
        assert_no_requests(&mut sink);
    }

    #[test]
    fn selecting_an_issue_from_a_draft_discards_the_draft() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            None,
            CursorRendering::Terminal,
            &mut sink,
        );
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), &mut sink);
        app.process_event(key_event(KeyCode::Char('n')), dispatcher.clone(), &mut sink);

        app.process_event(key_event(KeyCode::Char('y')), dispatcher.clone(), &mut sink);
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), &mut sink);
        app.process_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);
        assert!(new_issue_component(&app).is_some());

        app.process_event(key_event(KeyCode::Char('y')), dispatcher.clone(), &mut sink);
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), &mut sink);
        for code in [KeyCode::Char('l'), KeyCode::Enter] {
            app.process_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        assert!(new_issue_component(&app).is_none());
        assert_eq!(app.issue_component().unwrap().issue_id(), IssueId::new(1));
    }

    #[test]
    fn q_closes_issue_select_popup_without_dispatching_project_issues_action() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            None,
            CursorRendering::Terminal,
            &mut sink,
        );
        complete_initial_popup_page_fetch(&mut app, dispatcher.clone(), &mut sink);

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

        assert!(should_continue);
        assert!(app.popup_components.is_empty());
        assert_eq!(dispatcher.borrow().consume_actinos_len(), 0);
    }

    #[test]
    fn q_on_open_calendar_closes_only_calendar_without_quitting() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::DatePicker(
                DatePickerPopupComponent::new(None, Box::new(|_| {})),
            ))));
        for code in [KeyCode::Tab, KeyCode::Tab, KeyCode::Tab, KeyCode::Enter] {
            app.handle_key_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

        assert!(should_continue);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::DatePicker(_)
        ));
    }

    fn app_with_remote_journal_conflict_popup() -> (Rc<RefCell<Dispatcher>>, AppComponent<'static>)
    {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher_with_journals();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        {
            let mut dispatcher_ref = dispatcher.borrow_mut();
            dispatcher_ref.dispatch(Action::Journal(JournalAction::EditRemoteNotes {
                issue_id: IssueId::new(3),
                journal_id: JournalId::new(1),
                notes: "edited notes".to_string(),
            }));
            dispatcher_ref.consume_action();
            dispatcher_ref.dispatch(Action::Journal(JournalAction::StartRemoteUpload {
                issue_id: IssueId::new(3),
                journal_id: JournalId::new(1),
            }));
            dispatcher_ref.consume_action();
            let mut server_issue = crate::test_support::sample_parent_issue();
            server_issue.journals = crate::test_support::sample_parent_issue_journals();
            server_issue.journals[0].notes = "server notes".to_string();
            dispatcher_ref.dispatch(Action::Journal(JournalAction::DetectRemoteUploadConflict {
                journal_id: JournalId::new(1),
                issue: server_issue,
                children: crate::test_support::sample_parent_issue_children(),
            }));
            dispatcher_ref.consume_action();
        }
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        (dispatcher, app)
    }

    #[test]
    fn update_opens_remote_journal_conflict_popup_only_once() {
        let (dispatcher, mut app) = app_with_remote_journal_conflict_popup();

        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        assert_eq!(app.popup_components.len(), 1);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::RemoteJournalConflict { issue_id, journal_id, .. }
                if *issue_id == IssueId::new(3) && *journal_id == JournalId::new(1)
        ));
    }

    fn render_loaded_issue(
        cursor_rendering: CursorRendering,
    ) -> ratatui::Terminal<ratatui::backend::TestBackend> {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            cursor_rendering,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(AREA.width, AREA.height))
                .unwrap();
        terminal
            .draw(|frame| app.render(dispatcher.borrow().store(), frame, frame.area()))
            .unwrap();
        terminal
    }

    #[test]
    fn terminal_cursor_rendering_sets_terminal_cursor_without_reversing_cell() {
        let mut terminal = render_loaded_issue(CursorRendering::Terminal);

        // sample_parent_issueを開いた直後のheaderの初期cursor位置。
        assert_eq!(terminal.get_cursor_position().unwrap(), Position::new(4, 0));
        assert!(
            !terminal.backend().buffer()[(4, 0)]
                .modifier
                .contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn emulated_cursor_rendering_reverses_only_the_cursor_cell() {
        let terminal = render_loaded_issue(CursorRendering::Emulated);

        let buffer = terminal.backend().buffer();
        assert!(buffer[(4, 0)].modifier.contains(Modifier::REVERSED));
        assert!(!buffer[(3, 0)].modifier.contains(Modifier::REVERSED));
        assert!(!buffer[(5, 0)].modifier.contains(Modifier::REVERSED));
    }

    fn push_date_picker(app: &mut AppComponent<'static>) {
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::DatePicker(
                DatePickerPopupComponent::new(None, Box::new(|_| {})),
            ))));
    }

    fn push_spent_time_input(
        app: &mut AppComponent<'static>,
        dispatcher: &Rc<RefCell<Dispatcher>>,
    ) {
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::SpentTimeInput(
                SpentTimeInputPopupComponent::new(dispatcher.borrow().store()),
            ))));
    }

    #[test]
    fn q_on_date_picker_year_field_is_input_without_quitting() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        push_date_picker(&mut app);

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

        assert!(should_continue);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::DatePicker(_)
        ));
    }

    #[test]
    fn entering_an_attachment_requests_its_upload_and_closes_the_popup() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        sink.take_usecases();
        app.popup_components
            .push_back(Rc::new(RefCell::new(PopupComponent::AttachmentInput {
                destination: AttachmentDestination::Issue(3.into()),
                component: AttachmentInputPopupComponent::new(),
            })));
        for code in "a.png"
            .chars()
            .map(KeyCode::Char)
            .chain([KeyCode::Tab])
            .chain("/tmp/a.png".chars().map(KeyCode::Char))
            .chain([KeyCode::Enter])
        {
            app.handle_key_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        assert!(app.popup_components.is_empty());
        assert_eq!(
            sink.take_usecases(),
            vec![UsecaseRequest::StartAttachmentUpload {
                issue_id: 3.into(),
                filename: "a.png".to_string(),
                path: std::path::PathBuf::from("/tmp/a.png"),
            }]
        );
    }

    #[test]
    fn q_on_date_picker_buttons_closes_popup_without_quitting() {
        let mut sink = RequestSink::default();
        // Tab3回でカレンダーボタン、4回でキャンセルボタン
        for tab_count in [3, 4] {
            let dispatcher = loaded_dispatcher();
            let mut app = AppComponent::new(
                dispatcher.clone(),
                Some(3.into()),
                CursorRendering::Terminal,
                &mut sink,
            );
            push_date_picker(&mut app);
            for _ in 0..tab_count {
                app.handle_key_event(key_event(KeyCode::Tab), dispatcher.clone(), &mut sink);
            }

            let should_continue =
                app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

            assert!(should_continue, "tab_count={tab_count}");
            assert!(app.popup_components.is_empty(), "tab_count={tab_count}");
        }
    }

    #[test]
    fn q_on_navigating_spent_time_input_closes_popup_without_quitting() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        push_spent_time_input(&mut app, &dispatcher);

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

        assert!(should_continue);
        assert!(app.popup_components.is_empty());
    }

    #[test]
    fn q_while_editing_spent_time_memo_is_input_without_quitting() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        push_spent_time_input(&mut app, &dispatcher);
        // Activity -> Memo、Enterで編集モードに入る
        for code in [KeyCode::Char('j'), KeyCode::Enter] {
            app.handle_key_event(key_event(code), dispatcher.clone(), &mut sink);
        }

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

        assert!(should_continue);
        assert!(matches!(
            &*app.popup_components.back().unwrap().borrow(),
            PopupComponent::SpentTimeInput(_)
        ));
    }

    #[test]
    fn q_without_popup_quits() {
        let mut sink = RequestSink::default();
        let dispatcher = loaded_dispatcher();
        let mut app = AppComponent::new(
            dispatcher.clone(),
            Some(3.into()),
            CursorRendering::Terminal,
            &mut sink,
        );
        app.update(dispatcher.clone(), dispatcher.borrow().store(), AREA);

        let should_continue =
            app.handle_key_event(key_event(KeyCode::Char('q')), dispatcher.clone(), &mut sink);

        assert!(!should_continue);
    }
}
