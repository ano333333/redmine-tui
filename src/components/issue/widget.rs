use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;

use crate::widgets::Hr;

use super::body::widget::BodyWidget;
use super::children_list::widget::ChildrenListWidget;
use super::header::widget::HeaderWidget;
use super::journals_list::JournalsListWidget;
use super::property::widget::PropertyWidget;

pub struct IssueDetailWidgetState {
    /// グローバル座標のどのyから描画を始めるか
    pub offset_y: u16,
}

impl IssueDetailWidgetState {
    pub fn new() -> Self {
        Self { offset_y: 0 }
    }

    pub fn update(&mut self, cursor_global_position: Position, height: u16) {
        let cursor_y = cursor_global_position.y;
        if cursor_y < self.offset_y {
            self.offset_y = cursor_y;
        }
        if cursor_y >= self.offset_y + height {
            self.offset_y = (cursor_y + 1) - height;
        }
    }

    pub fn calc_cursor_area_position(
        &self,
        cursor_global_position: Position,
        area: Rect,
    ) -> Position {
        Position {
            x: area.x + cursor_global_position.x,
            y: (area.y + cursor_global_position.y).saturating_sub(self.offset_y),
        }
    }
}

pub struct IssueDetailWidget<'a> {
    header: HeaderWidget<'a>,
    property: PropertyWidget<'a>,
    body: BodyWidget<'a>,
    children_list: ChildrenListWidget<'a>,
    journals_list: JournalsListWidget<'a>,
    state: &'a IssueDetailWidgetState,
}

impl<'a> Widget for IssueDetailWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut line_count_sum = 0;
        let height = area.height;
        let width = area.width;
        let offset_y = self.state.offset_y;

        let mut area = area;

        let line_count = self.header.line_count(width) as u16;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            render_header_widget_to_buffer(buf, &mut area, self.header, line_count_sum, offset_y);
        }
        line_count_sum += line_count;

        let line_count = self.property.line_count(width) as u16;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            render_property_widget_to_buffer(
                buf,
                &mut area,
                self.property,
                line_count_sum,
                offset_y,
            )
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            Hr::default().render(area, buf);
            area.y += 1;
            area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.body.line_count(width) as u16;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            render_body_widget_to_buffer(buf, &mut area, self.body, line_count_sum, offset_y);
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            Hr::default().render(area, buf);
            area.y += 1;
            area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.children_list.line_count();
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            render_children_widget_to_buffer(
                buf,
                &mut area,
                self.children_list,
                line_count_sum,
                offset_y,
            );
        }
        line_count_sum += line_count;

        let line_count: u16 = 1;
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            Hr::default().render(area, buf);
            area.y += 1;
            area.height -= 1;
        }
        line_count_sum += line_count;

        let line_count = self.journals_list.line_count(width);
        if line_count_sum + line_count >= offset_y
            && line_count_sum < offset_y + height
            && area.height > 0
        {
            render_journals_list_widget_to_buffer(
                buf,
                &mut area,
                self.journals_list,
                line_count_sum,
                offset_y,
            );
        }
    }
}

impl<'a> IssueDetailWidget<'a> {
    pub fn new(
        header: HeaderWidget<'a>,
        property: PropertyWidget<'a>,
        body: BodyWidget<'a>,
        children_list: ChildrenListWidget<'a>,
        journals_list: JournalsListWidget<'a>,
        state: &'a IssueDetailWidgetState,
    ) -> Self {
        Self {
            header,
            property,
            body,
            children_list,
            journals_list,
            state,
        }
    }
}

/// HeaderComponentをBufferに描画し、書き込んだ領域を切り詰める
/// # Arguments
///
/// * `buffer` - 描画先のBuffer
/// * `frame_area` - `buffer`の描画領域
/// * `widget` - 描画するHeaderWidget
/// * `line_count_sum` - ここまでに書き込んだComponentの行数(line_count)の和
/// * `offset_y` - グローバル空間のどのy(行数)から書き始めるか
fn render_header_widget_to_buffer(
    buffer: &mut Buffer,
    area: &mut Rect,
    widget: HeaderWidget,
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
        let line_count = widget.line_count(area.width) as u16;
        widget.render(*area, buffer);
        area.y += min(line_count, area.height);
        area.height = area.height.saturating_sub(line_count);
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
        let line_count = widget.line_count(area.width) as u16;
        let buffer_area = Rect::new(0, 0, area.width, line_count);
        let mut temp_buffer = Buffer::empty(buffer_area);
        widget.render(buffer_area, &mut temp_buffer);

        // 一時Bufferの、グローバル空間でy=offset_yに位置する部分から後ろをframeに転写
        let overlapping_height = min(line_count_sum + line_count - offset_y, area.height);
        for y in 0..overlapping_height {
            for x in 0..area.width {
                let buffer_x = x;
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = area.x + x;
                let frame_y = area.y + y;
                let Some(buffer_cell) = temp_buffer.cell((buffer_x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = buffer.cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        area.y += overlapping_height;
        area.height -= overlapping_height;
    }
}

/// PropertyWidgetをBufferに描画し、書き込んだ領域を切り詰める
/// 詳しい説明はrender_header_widget_to_bufferを参照。
fn render_property_widget_to_buffer(
    buffer: &mut Buffer,
    area: &mut Rect,
    widget: PropertyWidget,
    line_count_sum: u16,
    offset_y: u16,
) {
    if offset_y <= line_count_sum {
        let line_count = widget.line_count(area.width) as u16;
        widget.render(*area, buffer);
        area.y += min(line_count, area.height);
        area.height = area.height.saturating_sub(line_count);
    } else {
        let line_count = widget.line_count(area.width) as u16;
        let buffer_area = Rect::new(0, 0, area.width, line_count);
        let mut temp_buffer = Buffer::empty(buffer_area);
        widget.render(buffer_area, &mut temp_buffer);

        let overlapping_height = min(line_count_sum + line_count - offset_y, area.height);
        for y in 0..overlapping_height {
            for x in 0..area.width {
                let buffer_x = x;
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = area.x + x;
                let frame_y = area.y + y;
                let Some(buffer_cell) = temp_buffer.cell((buffer_x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = buffer.cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        area.y += overlapping_height;
        area.height -= overlapping_height;
    }
}

/// BodyWidgetをBufferに描画し、書き込んだ領域を切り詰める
/// 詳しい説明はrender_header_widget_to_bufferを参照。
fn render_body_widget_to_buffer(
    buffer: &mut Buffer,
    area: &mut Rect,
    widget: BodyWidget,
    line_count_sum: u16,
    offset_y: u16,
) {
    if offset_y <= line_count_sum {
        let line_count = widget.line_count(area.width) as u16;
        widget.render(*area, buffer);
        area.y += min(line_count, area.height);
        area.height = area.height.saturating_sub(line_count);
    } else {
        let line_count = widget.line_count(area.width) as u16;
        let buffer_area = Rect::new(0, 0, area.width, line_count);
        let mut temp_buffer = Buffer::empty(buffer_area);
        widget.render(buffer_area, &mut temp_buffer);

        let overlapping_height = min(line_count_sum + line_count - offset_y, area.height);
        for y in 0..overlapping_height {
            for x in 0..area.width {
                let buffer_x = x;
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = area.x + x;
                let frame_y = area.y + y;
                let Some(buffer_cell) = temp_buffer.cell((buffer_x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = buffer.cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        area.y += overlapping_height;
        area.height -= overlapping_height;
    }
}

/// ChildrenListWidgetをBufferに描画し、書き込んだ領域を切り詰める
/// 詳しい説明はrender_header_widget_to_bufferを参照。
fn render_children_widget_to_buffer(
    buffer: &mut Buffer,
    area: &mut Rect,
    widget: ChildrenListWidget,
    line_count_sum: u16,
    offset_y: u16,
) {
    if offset_y <= line_count_sum {
        let line_count = widget.line_count() as u16;
        widget.render(*area, buffer);
        area.y += min(line_count, area.height);
        area.height = area.height.saturating_sub(line_count);
    } else {
        let line_count = widget.line_count() as u16;
        let buffer_area = Rect::new(0, 0, area.width, line_count);
        let mut temp_buffer = Buffer::empty(buffer_area);
        widget.render(buffer_area, &mut temp_buffer);

        let overlapping_height = min(line_count_sum + line_count - offset_y, area.height);
        for y in 0..overlapping_height {
            for x in 0..area.width {
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = area.x + x;
                let frame_y = area.y + y;
                let Some(buffer_cell) = temp_buffer.cell((x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = buffer.cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        area.y += overlapping_height;
        area.height -= overlapping_height;
    }
}

/// JournalsListWidgetをBufferに描画し、書き込んだ領域を切り詰める
/// 詳しい説明はrender_header_widget_to_bufferを参照。
fn render_journals_list_widget_to_buffer(
    buffer: &mut Buffer,
    area: &mut Rect,
    widget: JournalsListWidget,
    line_count_sum: u16,
    offset_y: u16,
) {
    if offset_y <= line_count_sum {
        let line_count = widget.line_count(area.width) as u16;
        widget.render(*area, buffer);
        area.y += min(line_count, area.height);
        area.height = area.height.saturating_sub(line_count);
    } else {
        let line_count = widget.line_count(area.width) as u16;
        let buffer_area = Rect::new(0, 0, area.width, line_count);
        let mut temp_buffer = Buffer::empty(buffer_area);
        widget.render(buffer_area, &mut temp_buffer);

        let overlapping_height = min(line_count_sum + line_count - offset_y, area.height);
        for y in 0..overlapping_height {
            for x in 0..area.width {
                let buffer_y = offset_y - line_count_sum + y;
                let frame_x = area.x + x;
                let frame_y = area.y + y;
                let Some(buffer_cell) = temp_buffer.cell((x, buffer_y)).cloned() else {
                    continue;
                };
                if let Some(frame_cell) = buffer.cell_mut((frame_x, frame_y)) {
                    *frame_cell = buffer_cell;
                }
            }
        }

        area.y += overlapping_height;
        area.height -= overlapping_height;
    }
}
