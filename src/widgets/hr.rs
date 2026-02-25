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
