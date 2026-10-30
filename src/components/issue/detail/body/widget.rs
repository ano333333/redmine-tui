use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Widget;

use super::super::markdown_view::{MarkdownImagePlacement, MarkdownImages, MarkdownView};

use crate::widgets::gutter::{GUTTER_WIDTH, Gutter, indented_area};
use crate::widgets::theme::FOCUS_BG;

/// 縦線を引かない末尾の余白行。次のブロックとの区切りになる。
const GUTTER_TRAILING_LINES: u16 = 1;

pub struct BodyWidgetState {
    view: MarkdownView,
}

impl BodyWidgetState {
    pub fn new() -> Self {
        Self {
            view: MarkdownView::new(),
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        let source = self.view.buffer();
        let width = min(source.area.width, area.width);
        let height = min(source.area.height, area.height);

        for y in 0..height {
            for x in 0..width {
                let Some(src_cell) = source.cell((x, y)).cloned() else {
                    continue;
                };
                let dst_x = area.x + x;
                let dst_y = area.y + y;
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    *dst_cell = src_cell;
                }
            }
        }

        if focused {
            for y in 0..height {
                for x in 0..width {
                    if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                        cell.set_bg(FOCUS_BG);
                    }
                }
            }
        }
    }

    pub fn line_count(&self, _: u16) -> usize {
        self.text_line_count() + GUTTER_TRAILING_LINES as usize
    }

    /// 末尾の余白行を含まない、本文そのものの行数。
    pub fn text_line_count(&self) -> usize {
        self.view.line_count() as usize
    }

    #[cfg(test)]
    pub fn update(&mut self, width: u16, body: &str) {
        self.update_with_images(width, body, &MarkdownImages::none());
    }

    /// 単独の画像の段落は、`images`で読み込んだ画像を表示し、表示できなければ代わりの文字にする。
    pub fn update_with_images(&mut self, width: u16, body: &str, images: &MarkdownImages) {
        // 本文は縦線の字下げを除いた幅で描く
        self.view
            .update(width.saturating_sub(GUTTER_WIDTH), body, images);
    }

    /// Widgetの左上から見た画像の位置。
    pub fn images(&self) -> Vec<MarkdownImagePlacement> {
        self.view
            .images()
            .iter()
            .map(|placement| MarkdownImagePlacement {
                position: Position::new(placement.position.x + GUTTER_WIDTH, placement.position.y),
                image: placement.image.clone(),
            })
            .collect()
    }
}

pub struct BodyWidget<'a> {
    state: &'a BodyWidgetState,
    focused: bool,
}

impl<'a> Widget for BodyWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let gutter_area = Rect::new(
            area.x,
            area.y,
            area.width.min(GUTTER_WIDTH),
            area.height
                .min((self.line_count(area.width) as u16).saturating_sub(GUTTER_TRAILING_LINES)),
        );
        let content_area = indented_area(area);

        Gutter::line().render(gutter_area, buf);
        self.state.render(content_area, buf, self.focused);
    }
}

impl<'a> BodyWidget<'a> {
    pub fn new(state: &'a BodyWidgetState, focused: bool) -> Self {
        Self { state, focused }
    }
    pub fn line_count(&self, width: u16) -> usize {
        self.state.line_count(width)
    }

    /// Widgetの左上から見た画像の位置。画像の行は空けて描くため、呼び出し側が画面へ直接描く。
    pub fn images(&self) -> Vec<MarkdownImagePlacement> {
        self.state.images()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_body_markdown_wide() {
        let mut state = BodyWidgetState::new();
        let body = "# Heading\n\n- first item with **bold**\n- second item with *italic*\n\nParagraph with [link](https://example.com).".to_string();
        state.update(32, &body);
        render_snapshot("body_markdown_wide", 32, 8, BodyWidget::new(&state, true));
    }

    #[test]
    fn snapshot_body_markdown_narrow_wrap() {
        let mut state = BodyWidgetState::new();
        let body = "# Heading\n\n- first item with **bold**\n- second item with *italic*\n\nParagraph text with [link](https://example.com) that should wrap.".to_string();
        state.update(18, &body);
        render_snapshot(
            "body_markdown_narrow_wrap",
            18,
            10,
            BodyWidget::new(&state, false),
        );
    }

    #[test]
    fn line_count_body_current_values() {
        let mut state = BodyWidgetState::new();
        let body = "# Heading\n\n- first item\n- second item\n\nParagraph text that should wrap."
            .to_string();
        // 本文は縦線の字下げを除いた幅で折り返し、末尾に余白1行が付く
        state.update(32, &body);
        assert_eq!(BodyWidget::new(&state, false).line_count(32), 8);

        state.update(18, &body);
        assert_eq!(BodyWidget::new(&state, false).line_count(18), 9);
    }

    #[test]
    fn line_count_body_changes_with_width_and_body() {
        let mut state = BodyWidgetState::new();
        let short_body = "Paragraph with enough words to wrap once in a narrow area.".to_string();
        let long_body = "Paragraph with enough words to wrap once in a narrow area, then expand into several additional wrapped lines for height growth.".to_string();

        state.update(32, &short_body);
        let wide_short = BodyWidget::new(&state, false).line_count(32);

        state.update(16, &short_body);
        let narrow_short = BodyWidget::new(&state, false).line_count(16);

        state.update(16, &long_body);
        let narrow_long = BodyWidget::new(&state, false).line_count(16);

        assert!(wide_short < narrow_short);
        assert!(narrow_short < narrow_long);
    }
}
