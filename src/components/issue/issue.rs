use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Offset, Position, Rect};

use crate::AppContainer;
use crate::app::{Dispatcher, Store};
use crate::widgets::Hr;

use super::body::EventProcessResult as BodyEventProcessResult;
use super::body::FocusEvent as BodyFocusEvent;
use super::body::IssueBodyComponent;
use super::children_list::EventProcessResult as ChildrenListEventProcessResult;
use super::children_list::FocusEvent as ChildrenListFocusEvent;
use super::children_list::IssueChildrenListComponent;
use super::header::EventProcessResult as HeaderEventProcessResult;
use super::header::FocusEvent as HeaderFocusEvent;
use super::header::IssueHeaderComponent;
use super::journal::JournalComponent;
use super::property::EventProcessResult as PropertyEventProcessResult;
use super::property::FocusTransitionEvent as PropertyFocusTransitionEvent;
use super::property::IssuePropertyComponent;

#[derive(PartialEq)]
enum FocusedComponent {
    Header,
    Property,
    Body,
    ChildrenList,
}

pub struct IssueDetailComponent {
    id: u16,
    header: IssueHeaderComponent,
    property: IssuePropertyComponent,
    body: IssueBodyComponent,
    children_list: IssueChildrenListComponent,
    journals: Vec<JournalComponent>,
    /// 描画横幅(render時のフレーム描画領域のwidthと一致)
    width: u16,
    /// 描画縦幅(render時のフレーム描画領域のheightと一致)
    height: u16,
    /// グローバル座標のどのyから描画を始めるか
    render_offset_y: u16,
    focused_component: FocusedComponent,
}

impl IssueDetailComponent {
    pub fn new(_: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        let size = AppContainer::size();
        match size {
            Ok((width, height)) => {
                let mut i = IssueDetailComponent {
                    id: issue_id,
                    header: IssueHeaderComponent::new(issue_id),
                    property: IssuePropertyComponent::new(issue_id),
                    body: IssueBodyComponent::new(issue_id),
                    children_list: IssueChildrenListComponent::new(issue_id),
                    journals: vec![],
                    width,
                    height,
                    render_offset_y: 0,
                    focused_component: FocusedComponent::Header,
                };
                i.header.focus_event(HeaderFocusEvent::Focused);
                i
            }
            Err(_) => panic!(),
        }
    }

    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        let issue = store.get_issue(self.id);
        match issue {
            None => {
                self.journals = vec![];
            }
            Some(issue) => {
                self.body.update(store, self.width);
                self.children_list.update(store);
                // FIXME: 差分更新
                self.journals = issue
                    .journal_ids
                    .iter()
                    .map(|i| JournalComponent::new(dispatcher.clone(), *i))
                    .collect();
                self.journals
                    .iter_mut()
                    .for_each(|journal| journal.update(dispatcher.clone(), store));
            }
        }

        let Position { y: cursor_y, .. } = self.calc_cursor_global_position(store);

        if cursor_y < self.render_offset_y {
            self.render_offset_y = cursor_y;
        }

        if cursor_y >= self.render_offset_y + self.height {
            self.render_offset_y = (cursor_y + 1).saturating_sub(self.height);
        }
    }

    pub fn render(&self, store: &Store, frame: &mut Frame, mut frame_area: Rect) {
        let mut line_count_sum: u16 = 0;
        let height = frame_area.height;
        let width = frame_area.width;
        let offset_y = self.render_offset_y;

        let mut cursor_position = self.calc_cursor_global_position(store);
        cursor_position.x += frame_area.x;
        cursor_position.y += frame_area.y;

        // TODO: ここのスクロール関係の描画処理を、もっとこう。。。
        let line_count = self.header.line_count(store);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) = self
                .header
                .render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count = self.property.line_count(store, width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) = self
                .property
                .render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            frame.render_widget(Hr::default(), frame_area);
            frame_area.y += 1;
            frame_area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.body.line_count(store, width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) = self.body.render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            frame.render_widget(Hr::default(), frame_area);
            frame_area.y += 1;
            frame_area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.children_list.line_count(store);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) =
                self.children_list
                    .render(store, frame_area.width, frame_area.height)
        {
            let buffer_area = Rect::new(
                0,
                offset_y.saturating_sub(line_count_sum),
                buffer.area.width,
                line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
            );
            render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            frame.render_widget(Hr::default(), frame_area);
            frame_area.y += 1;
            frame_area.height -= 1;
        }
        line_count_sum += line_count;

        for j in self.journals.iter() {
            let line_count = j.line_count(store, width);
            if line_count_sum + line_count >= offset_y
                && line_count_sum < offset_y + height
                && frame_area.height > 0
                && let Some(buffer) = j.render(store, frame_area.width, frame_area.height)
            {
                let buffer_area = Rect::new(
                    0,
                    offset_y.saturating_sub(line_count_sum),
                    buffer.area.width,
                    line_count.saturating_sub(offset_y.saturating_sub(line_count_sum)),
                );
                render_buffer_to_frame(frame, &mut frame_area, &buffer, buffer_area);
            }
            line_count_sum += line_count;
        }

        cursor_position.y -= self.render_offset_y;
        frame.set_cursor_position(cursor_position);
    }

    pub fn process_event(
        &mut self,
        event: crossterm::event::Event,
        _: Rc<RefCell<Dispatcher>>,
        store: &Store,
    ) {
        // FIXME:
        // process_eventでComponentのprocess_event呼び出しからその結果に基づくフォーカス処理を行っているが、
        // Storeの更新契機で子componentからイベントが来る可能性を踏まえ、updateがフォーカス関連を含めたイベントを返すようにしたい
        // ActionとしてStoreに流すか？(直接の親子関係があるComponent同士のイベント受け渡しにStoreを使いたくないが)
        match self.focused_component {
            FocusedComponent::Header => {
                let result = self.header.process_event(&event);
                if let Some(HeaderEventProcessResult::CursorLeavedFromBelow) = result {
                    self.header.focus_event(HeaderFocusEvent::Unfocused);
                    self.focused_component = FocusedComponent::Property;
                    self.property
                        .focus_event(PropertyFocusTransitionEvent::CursorEnteredFromAbove);
                }
            }
            FocusedComponent::Property => {
                let result = self.property.process_event(&event);
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
                        self.body.focus_event(
                            BodyFocusEvent::CursorEnteredFromAbove { x: 0 },
                            store,
                            self.width,
                        );
                    }
                    None => {}
                }
            }
            FocusedComponent::Body => {
                // FIXME: widthの受け渡し方、StoreにIssueDetailCompnent用の子Storeを作成？
                let result = self.body.process_event(&event, store, self.width);
                match result {
                    Some(BodyEventProcessResult::CursorLeavedFromAbove { .. }) => {
                        self.body
                            .focus_event(BodyFocusEvent::Unfocused, store, self.width);
                        self.focused_component = FocusedComponent::Property;
                        self.property
                            .focus_event(PropertyFocusTransitionEvent::CursorEnteredFromBelow);
                    }
                    Some(BodyEventProcessResult::CursorLeavedFromBelow { .. }) => {
                        self.body
                            .focus_event(BodyFocusEvent::Unfocused, store, self.width);
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
                        self.body.focus_event(
                            BodyFocusEvent::CursorEnteredFromBelow { x: 0 },
                            store,
                            self.width,
                        );
                    }
                    Some(ChildrenListEventProcessResult::CursorLeavedFromBelow) => {
                        // FIXME:
                        // journalsへのイベント受け渡し(self.journalsを差分更新するようにしないとフォーカス情報が消える、Propertyにカーソルが来た時どうするか？)
                    }
                    None => {}
                }
            }
        }
        if let Event::Resize(rows, cols) = event {
            self.width = rows;
            self.height = cols;
        }
    }

    /// IssueDetailComponentの全体から見たカーソル位置を計算する
    /// render時にクライアント座標への変換とFrame描画位置への加算を行うこと
    fn calc_cursor_global_position(&self, store: &Store) -> Position {
        let mut offset = Offset::default();

        if self.focused_component == FocusedComponent::Header {
            return self.header.get_cursor_position() + offset;
        }
        offset.y += self.header.line_count(store) as i32;

        if self.focused_component == FocusedComponent::Property {
            return self.property.get_cursor_position() + offset;
        }
        offset.y += self.property.line_count(store, self.width) as i32 + 1;

        if self.focused_component == FocusedComponent::Body {
            return self.body.get_cursor_position() + offset;
        }
        offset.y += self.body.line_count(store, self.width) as i32 + 1;

        if self.focused_component == FocusedComponent::ChildrenList {
            return self.children_list.get_cursor_position() + offset;
        }
        offset.y += self.children_list.line_count(store) as i32;

        // FIXME: コンポーネント追加(jorunals)
        Position { x: 0, y: 0 }
    }
}

/// BufferをFrame先頭にコピーし、コピー先の書き込んだ領域を切り詰める
///
/// # Arguments
///
/// * `frame` - コピー先のFrame
/// * `frame_area` - `frame`の領域
/// * `buffer` - コピー元のBuffer
/// * `buffer_area` - `buffer`の領域
fn render_buffer_to_frame(
    frame: &mut Frame,
    frame_area: &mut Rect,
    buffer: &Buffer,
    buffer_area: Rect,
) {
    let width = frame_area.width.min(buffer_area.width);
    let height = frame_area.height.min(buffer_area.height);

    let frame_buffer = frame.buffer_mut();
    for y in 0..height {
        for x in 0..width {
            let Some(src_cell) = buffer.cell((buffer_area.x + x, buffer_area.y + y)).cloned()
            else {
                continue;
            };
            let dst_x = frame_area.x + x;
            let dst_y = frame_area.y + y;
            if let Some(dst_cell) = frame_buffer.cell_mut((dst_x, dst_y)) {
                *dst_cell = src_cell;
            }
        }
    }

    frame_area.y += height;
    frame_area.height -= height;
}
