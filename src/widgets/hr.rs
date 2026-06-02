use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

pub struct Hr {
    char: String,
}

impl Hr {
    pub fn default() -> Self {
        Self {
            char: "─".to_string(),
        }
    }
}

impl Widget for Hr {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_string(
            area.x,
            area.y,
            self.char
                .repeat((area.width - 1) as usize / self.char.chars().count() + 1),
            Style::default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::render_snapshot;

    #[test]
    fn snapshot_hr_width_8() {
        render_snapshot("hr_width_8", 8, 1, Hr::default());
    }
}
