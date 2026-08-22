use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect, Size};
use ratatui::widgets::Widget;

pub struct VerticalScrollWidgetState {
    /// 全てのWidgetを含む仮想バッファを考えた時、どのyからクライアントに転写するか
    offset_y: u16,
}

impl VerticalScrollWidgetState {
    pub fn new() -> Self {
        Self { offset_y: 0 }
    }

    /// 現在の縦スクロール量を返す。
    /// FIXME: テストでprivateを見るためのメソッドの必要性、スナップショットでの判定で十分か？
    #[cfg(test)]
    pub fn offset_y(&self) -> u16 {
        self.offset_y
    }

    /// `VerticalScrollWidget`の`render`毎に`VerticalScrollWidgetState`を更新すること
    ///
    /// # Arguments
    ///
    /// * `cursor_global_position` - 全てのWidgetの仮想バッファから見たカーソル位置
    /// * `height` - VerticalScrollWidgetの表示行数
    pub fn update(&mut self, cursor_global_position: Position, height: u16) {
        let cursor_y = cursor_global_position.y;
        if cursor_y < self.offset_y {
            self.offset_y = cursor_y;
        } else if cursor_y >= self.offset_y.saturating_add(height) {
            self.offset_y = cursor_y.saturating_sub(height).saturating_add(1);
        }
    }

    /// 全てのWidgetの仮想バッファから見たカーソル位置を、
    /// クライアント座標に変換する。
    ///
    /// # Arguments
    ///
    /// * `cursor_global_position` - 全てのWidgetの仮想バッファから見たカーソル位置
    /// * `area` - このWidgetを描画するクライアント領域
    pub fn calc_cursor_area_position(
        &self,
        cursor_global_position: Position,
        area: Rect,
    ) -> Position {
        Position {
            x: area.x + cursor_global_position.x,
            y: area.y + cursor_global_position.y.saturating_sub(self.offset_y),
        }
    }
}

pub struct VerticalScrollWidget<'a> {
    state: &'a VerticalScrollWidgetState,
    buffer: Buffer,
    line_count_sum: u16,
}

impl<'a> VerticalScrollWidget<'a> {
    pub fn new(state: &'a VerticalScrollWidgetState, size: Size) -> Self {
        let buffer = Buffer::empty(Rect::from(size));
        Self {
            state,
            buffer,
            line_count_sum: 0,
        }
    }

    /// `VerticalScrollWidget::render`の前に、スクロール対象の子 Widget に対し上から順に呼び出す。
    ///
    /// # Arguments
    /// * `widget` - 子 Widget
    /// * `line_count` - `widget`の全てを描画するのに必要な行数
    pub fn render_widget<W: Widget>(&mut self, widget: W, line_count: u16) {
        let visible_start = self.state.offset_y;
        let visible_end = self.state.offset_y.saturating_add(self.buffer.area.height);
        let widget_start = self.line_count_sum;
        let widget_end = self.line_count_sum.saturating_add(line_count);

        if widget_end <= visible_start || widget_start >= visible_end {
            self.line_count_sum = self.line_count_sum.saturating_add(line_count);
            return;
        }

        // グローバル y 座標で見ると、
        // PropertyWidget は [self.line_count_sum, self.line_count_sum + line_count) を占める。
        // ここから、今回表示したい範囲 [offset_y, offset_y + self.buffer.height) との重なりだけを描画する。

        if self.state.offset_y <= self.line_count_sum {
            // Case 1: Widget の先頭から描ける場合
            //
            //     global y
            //        v
            //
            //     offset_y                                  +
            //                                               |
            //                                               |
            //   line_count_sum     +------------------+     | visible
            //                      |      Widget      |     |
            //                      |                  |     |
            //                      |                  |     +
            //                      |                  |
            //                      +------------------+
            //
            //   self.state.offset_y <= self.line_count_sum
            //   -> Widget の先頭は表示範囲内にあるので、
            //      Widget を先頭からそのまま Buffer に描ける
            let area_offset_y = self.line_count_sum - self.state.offset_y;
            let area = Rect::new(
                0,
                area_offset_y,
                self.buffer.area.width,
                self.buffer.area.height - area_offset_y,
            );
            widget.render(area, &mut self.buffer);
        } else {
            // Case 2: Widget の先頭が表示範囲より上にある場合
            //
            //     global y
            //        v
            //
            //   line_count_sum     +------------------+
            //                      |      Widget      |
            //                      |                  |
            //     offset_y         |                  |    +
            //                      |                  |    |
            //                      +------------------+    |
            //                                              | visible
            //                                              |
            //                                              +
            //
            //   self.line_count_sum < self.state.offset_y < self.line_count_sum + line_count
            //   -> Widget 上部は画面外に切れるので、
            //      一時 Buffer に描いてから
            //      (self.state.offset_y - self.line_count_sum) 行目以降だけを self.buffer に転写する
            let temp_buffer_area = Rect::new(0, 0, self.buffer.area.width, line_count);
            let mut temp_buffer = Buffer::empty(temp_buffer_area);
            widget.render(temp_buffer_area, &mut temp_buffer);

            let overlapping_height = min(widget_end - visible_start, temp_buffer_area.height);
            for y in 0..overlapping_height {
                for x in 0..temp_buffer_area.width {
                    let src_x = x;
                    let src_y = self.state.offset_y - self.line_count_sum + y;
                    let dst_x = x;
                    let dst_y = y;
                    let Some(src_cell) = temp_buffer.cell((src_x, src_y)).cloned() else {
                        continue;
                    };
                    if let Some(dst_cell) = self.buffer.cell_mut((dst_x, dst_y)) {
                        *dst_cell = src_cell;
                    }
                }
            }
        }
        self.line_count_sum = self.line_count_sum.saturating_add(line_count);
    }
}

impl<'a> Widget for VerticalScrollWidget<'a> {
    /// `Widget`としての描画関数
    /// `render_widget`で受け取っていた子 Widgetでクライアントに表示されるものを`buf`に描画する
    ///
    /// # example
    ///
    /// ```rust,ignore
    /// let widget1 = Widget1::new();
    /// let widget2 = Widget2::new();
    /// let widget3 = Widget3::new();
    ///
    /// let vs_widget = VerticalScrollWidget::new(&state, Size::new(30, 60));
    /// vs_widget.render_widget(widget1);
    /// vs_widget.render_widget(widget2);
    /// vs_widget.render_widget(widget3);
    ///
    /// frame.render(vs_widget, Rect::new(0, 0, 20, 20));
    /// ```
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = min(self.buffer.area.width, buf.area.width);
        let height = min(self.buffer.area.height, buf.area.height);
        for y in 0..height {
            for x in 0..width {
                let src_x = x;
                let src_y = y;
                let dst_x = area.x + x;
                let dst_y = area.y + y;
                let Some(src_cell) = self.buffer.cell((src_x, src_y)).cloned() else {
                    continue;
                };
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    *dst_cell = src_cell;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::{Position, Rect, Size};
    use ratatui::widgets::Paragraph;

    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn calc_cursor_area_position_without_scroll() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 3, y: 2 }, 5);

        assert_eq!(
            state.calc_cursor_area_position(Position { x: 3, y: 2 }, Rect::new(10, 20, 30, 5)),
            Position { x: 13, y: 22 }
        );
    }

    #[test]
    fn calc_cursor_area_position_after_scrolling_down() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 2, y: 4 }, 3);

        assert_eq!(state.offset_y(), 2);
        assert_eq!(
            state.calc_cursor_area_position(Position { x: 2, y: 4 }, Rect::new(10, 20, 30, 3)),
            Position { x: 12, y: 22 }
        );
    }

    #[test]
    fn update_scrolls_back_up_when_cursor_is_above_visible_area() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 5 }, 3);
        state.update(Position { x: 0, y: 1 }, 3);

        assert_eq!(state.offset_y(), 1);
        assert_eq!(
            state.calc_cursor_area_position(Position { x: 0, y: 1 }, Rect::new(10, 20, 30, 3)),
            Position { x: 10, y: 20 }
        );
    }

    #[test]
    fn update_keeps_offset_when_cursor_is_visible() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 5 }, 3);
        state.update(Position { x: 0, y: 4 }, 3);

        assert_eq!(state.offset_y(), 3);
    }

    #[test]
    fn snapshot_vertical_scroll_without_offset() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 0 }, 4);

        let widget = build_scroll_widget(&state, 4);
        render_snapshot("vertical_scroll_without_offset", 10, 4, widget);
    }

    #[test]
    fn snapshot_vertical_scroll_clips_top() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 4 }, 3);

        let widget = build_scroll_widget(&state, 3);
        render_snapshot("vertical_scroll_clips_top", 10, 3, widget);
    }

    #[test]
    fn snapshot_vertical_scroll_when_cursor_moves_below_visible_area() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 0 }, 3);
        state.update(Position { x: 0, y: 4 }, 3);
        assert_eq!(state.offset_y(), 2);

        let widget = build_scroll_widget(&state, 3);
        render_snapshot(
            "vertical_scroll_when_cursor_moves_below_visible_area",
            10,
            3,
            widget,
        );
    }

    #[test]
    fn snapshot_vertical_scroll_when_cursor_moves_above_visible_area() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 4 }, 3);
        state.update(Position { x: 0, y: 1 }, 3);
        assert_eq!(state.offset_y(), 1);

        let widget = build_scroll_widget(&state, 3);
        render_snapshot(
            "vertical_scroll_when_cursor_moves_above_visible_area",
            10,
            3,
            widget,
        );
    }

    #[test]
    fn snapshot_vertical_scroll_when_cursor_moves_down_inside_visible_area() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 4 }, 3);
        state.update(Position { x: 0, y: 1 }, 3);
        assert_eq!(state.offset_y(), 1);

        state.update(Position { x: 0, y: 2 }, 3);
        assert_eq!(state.offset_y(), 1);

        let widget = build_scroll_widget(&state, 3);
        render_snapshot(
            "vertical_scroll_when_cursor_moves_down_inside_visible_area",
            10,
            3,
            widget,
        );
    }

    #[test]
    fn snapshot_vertical_scroll_when_cursor_moves_up_inside_visible_area() {
        let mut state = VerticalScrollWidgetState::new();
        state.update(Position { x: 0, y: 4 }, 3);
        assert_eq!(state.offset_y(), 2);

        state.update(Position { x: 0, y: 2 }, 3);
        assert_eq!(state.offset_y(), 2);

        let widget = build_scroll_widget(&state, 3);
        render_snapshot(
            "vertical_scroll_when_cursor_moves_up_inside_visible_area",
            10,
            3,
            widget,
        );
    }

    fn build_scroll_widget(
        state: &VerticalScrollWidgetState,
        height: u16,
    ) -> VerticalScrollWidget<'_> {
        let mut widget = VerticalScrollWidget::new(state, Size::new(10, height));
        widget.render_widget(Paragraph::new("first"), 1);
        widget.render_widget(Paragraph::new("second\nthird\nfourth"), 3);
        widget.render_widget(Paragraph::new("fifth"), 1);
        widget
    }
}
