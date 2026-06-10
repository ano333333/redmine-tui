use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::{Offset, Position, Rect};
use ratatui::widgets::Widget;

use crate::AppContainer;
use crate::app::{Dispatcher, Store};
use crate::entities::Journal;

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

#[derive(PartialEq)]
enum FocusedComponent {
    Header,
    Property,
    Body,
    ChildrenList,
    JournalsList,
}

pub struct IssueDetailComponent {
    id: u16,
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
    pub fn new(_: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        let size = AppContainer::size();
        match size {
            Ok((width, height)) => {
                let mut i = IssueDetailComponent {
                    id: issue_id,
                    header: HeaderComponent::new(issue_id),
                    property: PropertyComponent::new(issue_id),
                    body: BodyComponent::new(issue_id, width, height),
                    children_list: ChildrenListComponent::new(issue_id),
                    journals_list: JournalsListComponent::new(),
                    widget_state: IssueDetailWidgetState::new(),
                    width,
                    height,
                    focused_component: FocusedComponent::Header,
                };
                i.header.focus_event(HeaderFocusEvent::Focused);
                i
            }
            Err(_) => panic!(),
        }
    }

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
    pub fn process_event(&mut self, event: crossterm::event::Event, _: Rc<RefCell<Dispatcher>>) {
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
                    None => {}
                }
            }
        }
        if let Event::Resize(cols, rows) = event {
            self.width = cols;
            self.height = rows;
        }
    }

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, _: Rc<RefCell<Dispatcher>>, store: &Store) {
        if let Some(issue) = store.get_issue(self.id) {
            self.body.update(issue, self.width);
            self.children_list.update(store);

            let journals = issue
                .journal_ids
                .iter()
                .map(|id| store.get_journal(*id))
                .filter(|journal| journal.is_some())
                .map(|journal| journal.unwrap())
                .collect::<Vec<&Journal>>();

            self.journals_list.update(journals, self.width);
        }

        self.widget_state
            .update(self.calc_cursor_global_position(store), self.height);
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, frame_area: Rect) {
        let widget = IssueDetailWidget::new(
            self.header.create_widget(store),
            self.property.create_widget(store),
            self.body.create_widget(),
            self.children_list.create_widget(store),
            self.journals_list.create_widget(),
            &self.widget_state,
        );
        widget.render(frame_area, frame.buffer_mut());

        let cursor_position = self
            .widget_state
            .calc_cursor_area_position(self.calc_cursor_global_position(store), frame_area);
        frame.set_cursor_position(cursor_position);
    }

    /// IssueDetailComponentの全体から見たカーソル位置を計算する
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
