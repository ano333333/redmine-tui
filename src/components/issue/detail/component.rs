use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Offset, Position, Rect};
use ratatui::widgets::Widget;

use crate::entities::Journal;
use crate::stores::{Dispatcher, JournalEntry, Store};
use crate::vos::{IssueId, JournalId, JournalKey};

use super::body::BodyComponent;
use super::body::EventProcessResult as BodyEventProcessResult;
use super::body::FocusEvent as BodyFocusEvent;
use super::children_list::ChildrenListComponent;
use super::children_list::EventProcessResult as ChildrenListEventProcessResult;
use super::children_list::FocusEvent as ChildrenListFocusEvent;
use super::header::EventProcessResult as HeaderEventProcessResult;
use super::header::FocusEvent as HeaderFocusEvent;
use super::header::HeaderComponent;
use super::journals_list::EventProcessResult as JournalsListEventProcessResult;
use super::journals_list::FocusEvent as JournalsListFocusEvent;
use super::journals_list::JournalsListComponent;
use super::property::EventProcessResult as PropertyEventProcessResult;
use super::property::FocusEvent as PropertyFocusTransitionEvent;
use super::property::PropertyComponent;
use super::{IssueDetailWidget, IssueDetailWidgetState};

#[derive(Debug, PartialEq, Eq)]
pub enum EventProcessResult {
    EditIssueBodyRequested { id: IssueId, body: String },
    EditJournalRequested { id: JournalId, notes: String },
    OpenIssueStatusPopup,
    OpenAssignedToPopup,
    OpenTargetVersionPopup,
    OpenStartDatePopup,
    OpenDueDatePopup,
    OpenDoneRatioPopup,
    OpenSpentTimeInputPopup,
    OpenCategoryPopup,
    StartIssueUpload,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stores::IssueAction;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn dispatcher() -> Rc<RefCell<Dispatcher>> {
        Rc::new(RefCell::new(Dispatcher::new()))
    }

    fn dispatcher_with_issue() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = dispatcher();
        dispatcher
            .borrow_mut()
            .dispatch(IssueAction::Load { id: 3.into() });
        dispatcher.borrow_mut().consume_action();
        dispatcher
    }

    #[test]
    fn process_event_y_is_not_owned_by_issue_detail() {
        let dispatcher = dispatcher_with_issue();
        let mut component = IssueDetailComponent::new(3);

        assert!(
            component
                .process_event(key_event(KeyCode::Char('y')), dispatcher)
                .is_none()
        );
    }

    /// Property(15行) -> Body -> ChildrenList -> JournalsList(先頭Journalのdetail)
    /// の順にフォーカスを送り、JournalsList内の1件目JournalのNotes位置に到達させる
    const J_PRESSES_TO_FIRST_JOURNAL_NOTES: usize = 54;

    fn dispatcher_with_issue_and_journals() -> Rc<RefCell<Dispatcher>> {
        let dispatcher = dispatcher();
        {
            let mut d = dispatcher.borrow_mut();
            crate::test_support::dispatch_fixture_entity_actions(&mut d);
            d.dispatch(IssueAction::Load { id: 3.into() });
            d.dispatch(crate::stores::Action::LoadJournal { id: 1.into() });
            d.dispatch(crate::stores::Action::LoadJournal { id: 2.into() });
            d.dispatch(crate::stores::Action::LoadJournal { id: 3.into() });
            while d.consume_actinos_len() > 0 {
                d.consume_action();
            }
        }
        dispatcher
    }

    #[test]
    fn process_event_e_on_journals_list_notes_returns_edit_journal_requested() {
        let dispatcher = dispatcher_with_issue_and_journals();
        let mut component = IssueDetailComponent::new(3);
        component.update(dispatcher.clone(), dispatcher.borrow().store(), (80, 24));

        for _ in 0..J_PRESSES_TO_FIRST_JOURNAL_NOTES {
            component.process_event(key_event(KeyCode::Char('j')), dispatcher.clone());
            component.update(dispatcher.clone(), dispatcher.borrow().store(), (80, 24));
        }

        let result = component.process_event(key_event(KeyCode::Char('e')), dispatcher.clone());

        match result {
            Some(EventProcessResult::EditJournalRequested { id, notes }) => {
                assert_eq!(id, JournalId::new(1));
                assert_eq!(notes, "");
            }
            _ => panic!("expected edit journal request"),
        }
    }
}

#[derive(PartialEq)]
enum FocusedComponent {
    Header,
    Property,
    Body,
    ChildrenList,
    JournalsList,
}

pub struct IssueDetailComponent {
    id: IssueId,
    header: HeaderComponent,
    property: PropertyComponent,
    body: BodyComponent,
    children_list: ChildrenListComponent,
    journals_list: JournalsListComponent,
    widget_state: IssueDetailWidgetState,
    /// 描画横幅(render時のフレーム描画領域のwidthと一致)
    width: u16,
    /// 描画縦幅(render時のフレーム描画領域のheightと一致)
    height: u16,
    focused_component: FocusedComponent,
}

impl IssueDetailComponent {
    pub fn new(issue_id: impl Into<IssueId>) -> Self {
        let issue_id = issue_id.into();
        let mut i = IssueDetailComponent {
            id: issue_id,
            header: HeaderComponent::new(issue_id),
            property: PropertyComponent::new(issue_id),
            body: BodyComponent::new(issue_id),
            children_list: ChildrenListComponent::new(issue_id),
            journals_list: JournalsListComponent::new(),
            widget_state: IssueDetailWidgetState::new(),
            // 初期化の直後のupdateに初期化を遅延する
            width: 0,
            height: 0,
            focused_component: FocusedComponent::Header,
        };
        i.header.focus_event(HeaderFocusEvent::Focused);
        i
    }

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
    pub fn process_event(
        &mut self,
        event: crossterm::event::Event,
        _dispatcher: Rc<RefCell<Dispatcher>>,
    ) -> Option<EventProcessResult> {
        if let Event::Key(key) = &event {
            match key.code {
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Some(EventProcessResult::StartIssueUpload);
                }
                _ => {}
            }
        }

        // FIXME:
        // process_eventでComponentのprocess_event呼び出しからその結果に基づくフォーカス処理を行っているが、
        // Storeの更新契機で子componentからイベントが来る可能性を踏まえ、updateがフォーカス関連を含めたイベントを返すようにしたい
        // ActionとしてStoreに流すか？(直接の親子関係があるComponent同士のイベント受け渡しにStoreを使いたくないが)
        match self.focused_component {
            FocusedComponent::Header => {
                let result = self.header.process_event(event.clone());
                if let Some(HeaderEventProcessResult::CursorLeavedFromBelow) = result {
                    self.header.focus_event(HeaderFocusEvent::Unfocused);
                    self.focused_component = FocusedComponent::Property;
                    self.property
                        .focus_event(PropertyFocusTransitionEvent::CursorEnteredFromAbove);
                }
            }
            FocusedComponent::Property => {
                let result = self.property.process_event(event.clone());
                match result {
                    Some(PropertyEventProcessResult::CursorLeavedFromAbove) => {
                        self.property
                            .focus_event(PropertyFocusTransitionEvent::Unfocused);
                        self.focused_component = FocusedComponent::Header;
                        self.header
                            .focus_event(HeaderFocusEvent::CursorEnteredFromBelow);
                    }
                    Some(PropertyEventProcessResult::CursorLeavedFromBelow) => {
                        self.property
                            .focus_event(PropertyFocusTransitionEvent::Unfocused);
                        self.focused_component = FocusedComponent::Body;
                        self.body
                            .focus_event(BodyFocusEvent::CursorEnteredFromAbove { x: 0 });
                    }
                    Some(PropertyEventProcessResult::OpenIssueStatusPopup) => {
                        return Some(EventProcessResult::OpenIssueStatusPopup);
                    }
                    Some(PropertyEventProcessResult::OpenAssignedToPopup) => {
                        return Some(EventProcessResult::OpenAssignedToPopup);
                    }
                    Some(PropertyEventProcessResult::OpenTargetVersionPopup) => {
                        return Some(EventProcessResult::OpenTargetVersionPopup);
                    }
                    Some(PropertyEventProcessResult::OpenStartDatePopup) => {
                        return Some(EventProcessResult::OpenStartDatePopup);
                    }
                    Some(PropertyEventProcessResult::OpenDueDatePopup) => {
                        return Some(EventProcessResult::OpenDueDatePopup);
                    }
                    Some(PropertyEventProcessResult::OpenDoneRatioPopup) => {
                        return Some(EventProcessResult::OpenDoneRatioPopup);
                    }
                    Some(PropertyEventProcessResult::OpenSpentTimeInputPopup) => {
                        return Some(EventProcessResult::OpenSpentTimeInputPopup);
                    }
                    Some(PropertyEventProcessResult::OpenCategoryPopup) => {
                        return Some(EventProcessResult::OpenCategoryPopup);
                    }
                    None => {}
                }
            }
            FocusedComponent::Body => {
                // FIXME: widthの受け渡し方、StoreにIssueDetailCompnent用の子Storeを作成？
                let result = self.body.process_event(event.clone());
                match result {
                    Some(BodyEventProcessResult::CursorLeavedFromAbove { .. }) => {
                        self.body.focus_event(BodyFocusEvent::Unfocused);
                        self.focused_component = FocusedComponent::Property;
                        self.property
                            .focus_event(PropertyFocusTransitionEvent::CursorEnteredFromBelow);
                    }
                    Some(BodyEventProcessResult::CursorLeavedFromBelow { .. }) => {
                        self.body.focus_event(BodyFocusEvent::Unfocused);
                        self.focused_component = FocusedComponent::ChildrenList;
                        self.children_list
                            .focus_event(ChildrenListFocusEvent::CursorEnteredFromAbove);
                    }
                    Some(BodyEventProcessResult::EditRequested { id, body }) => {
                        return Some(EventProcessResult::EditIssueBodyRequested { id, body });
                    }
                    None => {}
                }
            }
            FocusedComponent::ChildrenList => {
                let result = self.children_list.process_event(&event);
                match result {
                    Some(ChildrenListEventProcessResult::CursorLeavedFromAbove) => {
                        self.children_list
                            .focus_event(ChildrenListFocusEvent::Unfocused);
                        self.focused_component = FocusedComponent::Body;
                        self.body
                            .focus_event(BodyFocusEvent::CursorEnteredFromBelow { x: 0 });
                    }
                    Some(ChildrenListEventProcessResult::CursorLeavedFromBelow) => {
                        self.children_list
                            .focus_event(ChildrenListFocusEvent::Unfocused);
                        self.focused_component = FocusedComponent::JournalsList;
                        self.journals_list
                            .focus_event(JournalsListFocusEvent::CursorEnteredFromAbove { x: 0 });
                    }
                    None => {}
                }
            }
            FocusedComponent::JournalsList => {
                let result = self.journals_list.process_event(event.clone());
                match result {
                    Some(JournalsListEventProcessResult::CursorLeavedFromBelow) => {}
                    Some(JournalsListEventProcessResult::CursorLeavedFromAbove) => {
                        self.journals_list
                            .focus_event(JournalsListFocusEvent::Unfocused);
                        self.focused_component = FocusedComponent::ChildrenList;
                        self.children_list
                            .focus_event(ChildrenListFocusEvent::CursorEnteredFromBelow);
                    }
                    Some(JournalsListEventProcessResult::EditRequested { id, notes }) => {
                        return Some(EventProcessResult::EditJournalRequested { id, notes });
                    }
                    None => {}
                }
            }
        }
        None
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, _: Rc<RefCell<Dispatcher>>, store: &Store, frame_size: (u16, u16)) {
        (self.width, self.height) = frame_size;
        if let Some((issue, _)) = store.get_issue(self.id) {
            self.body.update(issue, self.width);
            self.children_list.update(store);

            let journals = issue
                .journal_keys
                .iter()
                .filter_map(|key| match key {
                    JournalKey::Remote(_) => store.get_journal_entry(*key),
                    JournalKey::Local(_) => None,
                })
                .filter_map(|entry| match entry {
                    JournalEntry::Remote { journal, .. } => Some(journal),
                    JournalEntry::Local { .. } => None,
                })
                .collect::<Vec<&Journal>>();

            self.journals_list.update(journals, self.width);
        }

        self.widget_state.update(
            self.calc_cursor_global_position(store),
            self.height,
            self.header.line_count(store, self.width),
        );
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, frame_area: Rect) {
        self.create_widget(store)
            .render(frame_area, frame.buffer_mut());
    }

    pub fn create_widget<'a>(&'a self, store: &'a Store) -> IssueDetailWidget<'a> {
        IssueDetailWidget::new(
            self.header.create_widget(store),
            self.property.create_widget(store),
            self.body.create_widget(),
            self.children_list.create_widget(store),
            self.journals_list.create_widget(store),
            &self.widget_state,
        )
    }

    pub fn calc_cursor_position(&self, store: &Store, frame_area: Rect) -> Position {
        let header_height = self.header.line_count(store, frame_area.width);
        self.widget_state.calc_cursor_area_position(
            self.calc_cursor_global_position(store),
            frame_area,
            header_height,
        )
    }

    pub fn issue_id(&self) -> IssueId {
        self.id
    }

    /// IssueDetailComponentの全体を含む仮想バッファから見たカーソル位置を計算する(HeaderWidget含)
    /// render時にクライアント座標への変換とFrame描画位置への加算を行うこと
    fn calc_cursor_global_position(&self, store: &Store) -> Position {
        let mut offset = Offset::default();

        if self.focused_component == FocusedComponent::Header {
            return self.header.get_cursor_position() + offset;
        }
        offset.y += self.header.line_count(store, self.width) as i32;

        if self.focused_component == FocusedComponent::Property {
            return self.property.get_cursor_position() + offset;
        }
        offset.y += self.property.line_count(store, self.width) as i32 + 1;

        if self.focused_component == FocusedComponent::Body {
            return self.body.get_cursor_position() + offset;
        }
        offset.y += self.body.line_count(self.width) as i32 + 1;

        if self.focused_component == FocusedComponent::ChildrenList {
            return self.children_list.get_cursor_position() + offset;
        }
        offset.y += self.children_list.line_count(store) as i32 + 1;

        if self.focused_component == FocusedComponent::JournalsList {
            return self.journals_list.get_cursor_position(self.width) + offset;
        }
        offset.y += self.journals_list.line_count(self.width) as i32 + 2;

        Position { x: 0, y: 0 }
    }
}
