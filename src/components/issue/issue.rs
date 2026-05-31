use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Offset, Position, Rect};

use crate::AppContainer;
use crate::app::{Dispatcher, Store};
use crate::widgets::Hr;

use super::body::BodyComponent;
use super::body::EventProcessResult as BodyEventProcessResult;
use super::body::FocusEvent as BodyFocusEvent;
use super::children_list::EventProcessResult as ChildrenListEventProcessResult;
use super::children_list::FocusEvent as ChildrenListFocusEvent;
use super::children_list::IssueChildrenListComponent;
use super::header::EventProcessResult as HeaderEventProcessResult;
use super::header::FocusEvent as HeaderFocusEvent;
use super::header::HeaderComponent;
use super::journals_list::EventProcessResult as JournalsListEventProcessResult;
use super::journals_list::FocusEvent as JournalsListFocusEvent;
use super::journals_list::JournalsListComponent;
use super::property::EventProcessResult as PropertyEventProcessResult;
use super::property::FocusEvent as PropertyFocusTransitionEvent;
use super::property::PropertyComponent;

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
    children_list: IssueChildrenListComponent,
    journals_list: JournalsListComponent,
    /// 描画横幅(render時のフレーム描画領域のwidthと一致)
    width: u16,
    /// 描画縦幅(render時のフレーム描画領域のheightと一致)
    height: u16,
    /// グローバル座標のどのyから描画を始めるか
    render_offset_y: u16,
    focused_component: FocusedComponent,
}

impl IssueDetailComponent {
    // TODO: journalsをJournalListComponentに置き換え
    pub fn new(_: Rc<RefCell<Dispatcher>>, issue_id: u16) -> Self {
        let size = AppContainer::size();
        match size {
            Ok((width, height)) => {
                let mut i = IssueDetailComponent {
                    id: issue_id,
                    header: HeaderComponent::new(issue_id),
                    property: PropertyComponent::new(issue_id),
                    body: BodyComponent::new(issue_id, width, height),
                    children_list: IssueChildrenListComponent::new(issue_id),
                    journals_list: JournalsListComponent::new(issue_id),
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

    /// crosstermの同期イベントを処理する。updateとrenderがこの順で後続する
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
                        self.journals_list.focus_event(
                            store,
                            JournalsListFocusEvent::CursorEnteredFromAbove,
                            self.width,
                        );
                    }
                    None => {}
                }
            }
            FocusedComponent::JournalsList => {
                let result = self.journals_list.process_event(&event, store, self.width);
                match result {
                    Some(JournalsListEventProcessResult::CursorLeavedFromBelow) => {}
                    Some(JournalsListEventProcessResult::CursorLeavedFromAbove) => {
                        self.journals_list.focus_event(
                            store,
                            JournalsListFocusEvent::Unfocused,
                            self.width,
                        );
                        self.focused_component = FocusedComponent::ChildrenList;
                        self.children_list
                            .focus_event(ChildrenListFocusEvent::CursorEnteredFromBelow);
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

    /// Storeの更新を取得しComponentの状態を更新する。renderが後続する。
    pub fn update(&mut self, dispatcher: Rc<RefCell<Dispatcher>>, store: &Store) {
        if store.get_issue(self.id).is_some() {
            self.body.update(store, self.width);
            self.children_list.update(store);

            self.journals_list.update(store, self.width);
        }

        let Position { y: cursor_y, .. } = self.calc_cursor_global_position(store);

        if cursor_y < self.render_offset_y {
            self.render_offset_y = cursor_y;
        }

        if cursor_y >= self.render_offset_y + self.height {
            self.render_offset_y = (cursor_y + 1).saturating_sub(self.height);
        }
    }

    /// Componentをframeのarea範囲内に描画する。
    pub fn render(&self, store: &Store, frame: &mut Frame, mut frame_area: Rect) {
        let mut line_count_sum: u16 = 0;
        let height = frame_area.height;
        let width = frame_area.width;
        let offset_y = self.render_offset_y;

        let mut cursor_position = self.calc_cursor_global_position(store);
        cursor_position.x += frame_area.x;
        cursor_position.y += frame_area.y;

        // TODO: ここのスクロール関係の描画処理を、もっとこう。。。
        let line_count = self.header.line_count(store, width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            render_header_component_to_frame(
                store,
                frame,
                &mut frame_area,
                &self.header,
                line_count_sum,
                offset_y,
            );
        }
        line_count_sum += line_count;

        let line_count = self.property.line_count(store, width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
        {
            render_property_component_to_frame(
                store,
                frame,
                &mut frame_area,
                &self.property,
                line_count_sum,
                offset_y,
            )
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
        {
            render_body_component_to_frame(
                store,
                frame,
                &mut frame_area,
                &self.body,
                line_count_sum,
                offset_y,
            );
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

        let line_count = self.journals_list.line_count(store, self.width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && frame_area.height > 0
            && let Some(buffer) =
                self.journals_list
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

        cursor_position.y -= self.render_offset_y;
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
        offset.y += self.body.line_count(store, self.width) as i32 + 1;

        if self.focused_component == FocusedComponent::ChildrenList {
            return self.children_list.get_cursor_position() + offset;
        }
        offset.y += self.children_list.line_count(store) as i32 + 1;

        if self.focused_component == FocusedComponent::JournalsList {
            return self.journals_list.get_cursor_position(store, self.width) + offset;
        }
        offset.y += self.journals_list.line_count(store, self.width) as i32 + 2;

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

/// HeaderComponentをFrameに描画し、書き込んだ領域を切り詰める
/// # Arguments
///
/// * `store` - Store
/// * `frame` - 描画先のFrame
/// * `frame_area` - `frame`の描画領域
/// * `component` - 描画するHeaderComponent
/// * `line_count_sum` - ここまでに書き込んだComponentの行数(line_count)の和
/// * `offset_y` - グローバル空間のどのy(行数)から書き始めるか
fn render_header_component_to_frame(
    store: &Store,
    frame: &mut Frame,
    frame_area: &mut Rect,
    component: &HeaderComponent,
    line_count_sum: u16,
    offset_y: u16,
) {
    // グローバル y 座標で見ると、
    // HeaderComponent は [line_count_sum, line_count_sum + line_count) を占める。
    // ここから、今回表示したい範囲 [offset_y, +inf) との重なりだけを描画する。

    // Case 1: Header の先頭から描ける場合
    //
    //     global y
    //        v
    //
    //     offset_y                                  +
    //                                               |
    //                                               |
    //   line_count_sum     +------------------+     | visible
    //                      |      Header      |     |
    //                      |                  |     |
    //                      |                  |     +
    //                      |                  |
    //                      +------------------+
    //
    //   offset_y <= line_count_sum
    //   -> Header の先頭は表示範囲内にあるので、
    //      Header を先頭からそのまま Frame に描ける
    if offset_y <= line_count_sum {
        component.render(store, *frame_area, frame.buffer_mut());
        let line_count = component.line_count(store, frame_area.width);
        frame_area.y += min(line_count, frame_area.height);
        frame_area.height = frame_area.height.saturating_sub(line_count);
    }
    // Case 2: Header の先頭が表示範囲より上にある場合
    //
    //     global y
    //        v
    //
    //   line_count_sum     +------------------+
    //                      |      Header      |
    //                      |                  |
    //   offset_y           |                  |    +
    //                      |                  |    |
    //                      +------------------+    |
    //                                              | visible
    //                                              |
    //                                              +
    //
    //   line_count_sum < offset_y < line_count_sum + line_count
    //   -> Header 上部は画面外に切れるので、
    //      一時 Buffer に描いてから
    //      (offset_y - line_count_sum) 行目以降だけを Frame に転写する
    else {
        // componentを一時Bufferに書き出す必要がある
        let line_count = component.line_count(store, frame_area.width);
        let buffer_area = Rect::new(0, 0, frame_area.width, line_count);
        let mut buffer = Buffer::empty(buffer_area);
        component.render(store, buffer_area, &mut buffer);

        // 一時Bufferの、グローバル空間でy=offset_yに位置する部分から後ろをframeに転写
        let overlapping_height = min(line_count_sum + line_count - offset_y, frame_area.height);
        for y in 0..overlapping_height {
            for x in 0..frame_area.width {
                let buffer_x = x;
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = frame_area.x + x;
                let frame_y = frame_area.y + y;
                let Some(buffer_cell) = buffer.cell((buffer_x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = frame.buffer_mut().cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        frame_area.y += overlapping_height;
        frame_area.height -= overlapping_height;
    }
}

/// PropertyComponentをFrameに描画し、書き込んだ領域を切り詰める
/// 詳しい説明はrender_header_component_to_frameを参照。
fn render_property_component_to_frame(
    store: &Store,
    frame: &mut Frame,
    frame_area: &mut Rect,
    component: &PropertyComponent,
    line_count_sum: u16,
    offset_y: u16,
) {
    // FIXME:子componentのtrait等による共通化ができていないので、render_*_component_to_frameを毎度定義する必要がある。
    if offset_y <= line_count_sum {
        component.render(store, *frame_area, frame.buffer_mut());
        let line_count = component.line_count(store, frame_area.width);
        frame_area.y += min(line_count, frame_area.height);
        frame_area.height = frame_area.height.saturating_sub(line_count);
    } else {
        let line_count = component.line_count(store, frame_area.width);
        let buffer_area = Rect::new(0, 0, frame_area.width, line_count);
        let mut buffer = Buffer::empty(buffer_area);
        component.render(store, buffer_area, &mut buffer);

        let overlapping_height = min(line_count_sum + line_count - offset_y, frame_area.height);
        for y in 0..overlapping_height {
            for x in 0..frame_area.width {
                let buffer_x = x;
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = frame_area.x + x;
                let frame_y = frame_area.y + y;
                let Some(buffer_cell) = buffer.cell((buffer_x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = frame.buffer_mut().cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        frame_area.y += overlapping_height;
        frame_area.height -= overlapping_height;
    }
}

/// BodyComponentをFrameに描画し、書き込んだ領域を切り詰める
/// 詳しい説明はrender_header_component_to_frameを参照。
fn render_body_component_to_frame(
    store: &Store,
    frame: &mut Frame,
    frame_area: &mut Rect,
    component: &BodyComponent,
    line_count_sum: u16,
    offset_y: u16,
) {
    // FIXME:子componentのtrait等による共通化ができていないので、render_*_component_to_frameを毎度定義する必要がある。
    if offset_y <= line_count_sum {
        component.render(store, *frame_area, frame.buffer_mut());
        let line_count = component.line_count(store, frame_area.width);
        frame_area.y += min(line_count, frame_area.height);
        frame_area.height = frame_area.height.saturating_sub(line_count);
    } else {
        let line_count = component.line_count(store, frame_area.width);
        let buffer_area = Rect::new(0, 0, frame_area.width, line_count);
        let mut buffer = Buffer::empty(buffer_area);
        component.render(store, buffer_area, &mut buffer);

        let overlapping_height = min(line_count_sum + line_count - offset_y, frame_area.height);
        for y in 0..overlapping_height {
            for x in 0..frame_area.width {
                let buffer_x = x;
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = frame_area.x + x;
                let frame_y = frame_area.y + y;
                let Some(buffer_cell) = buffer.cell((buffer_x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = frame.buffer_mut().cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        frame_area.y += overlapping_height;
        frame_area.height -= overlapping_height;
    }
}
