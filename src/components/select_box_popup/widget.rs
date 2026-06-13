use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

const FOCUS_BG: Color = Color::Rgb(0x1A, 0x33, 0x22);

pub struct SelectBoxPopupWidget<'a> {
    pub names: &'a [String],
    pub focused_index: usize,
}

impl<'a> SelectBoxPopupWidget<'a> {
    pub fn new(names: &'a [String], focused_index: usize) -> Self {
        Self {
            names,
            focused_index,
        }
    }

    pub fn line_count(&self, _: u16) -> usize {
        self.names.len().saturating_add(2)
    }
}

impl Widget for SelectBoxPopupWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height < 2 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let visible_count = inner.height.min(self.names.len() as u16) as usize;

        for (row, name) in self.names.iter().take(visible_count).enumerate() {
            let y = inner.y + row as u16;
            let style = if row == self.focused_index {
                Style::default().bg(FOCUS_BG)
            } else {
                Style::default()
            };
            render_single_line(buf, inner.x, y, inner.width, name, style);
        }
    }
}

fn render_single_line(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    let blank = " ".repeat(width as usize);
    buf.set_line(x, y, &Line::styled(blank, style), width);

    let clipped = text.chars().take(width as usize).collect::<String>();
    buf.set_line(x, y, &Line::styled(clipped, style), width);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_select_box_popup_renders_focus_and_clips_without_wrap() {
        let names = vec![
            "New".to_string(),
            "In Progress".to_string(),
            "Waiting for external review with long label".to_string(),
            "Closed".to_string(),
        ];

        render_snapshot(
            "select_box_popup_focus_and_clip",
            20,
            6,
            SelectBoxPopupWidget::new(&names, 2),
        );
    }

    #[test]
    fn line_count_includes_borders() {
        let names = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let widget = SelectBoxPopupWidget::new(&names, 0);
        assert_eq!(widget.line_count(10), 5);
    }
}
