use std::cmp::max;

use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders},
};

const APP_WIDTH_MIN: usize = 40;
const APP_HEIGHT_MIN: usize = 40;
struct App {
    width: usize,
    height: usize,
}

impl App {
    pub fn handle_key_event(&mut self, event: Event) -> bool {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Char('q') {
                return false;
            }
            match key.code {
                KeyCode::Left => {
                    self.width = max(self.width - 1, APP_WIDTH_MIN);
                }
                KeyCode::Right => {
                    self.width = self.width.saturating_add(1);
                }
                KeyCode::Up => {
                    self.height = max(self.height - 1, APP_HEIGHT_MIN);
                }
                KeyCode::Down => {
                    self.height = self.height.saturating_add(1);
                }
                _ => {}
            }
        }
        true
    }
}

fn main() {
    let mut terminal = ratatui::init();
    let mut app = App {
        width: 80,
        height: 80,
    };
    loop {
        terminal
            .draw(|f| draw(f, &app))
            .expect("failed to draw frame");
        if !app.handle_key_event(event::read().expect("failed to read event")) {
            break;
        }
    }
    ratatui::restore();
}

fn draw(frame: &mut Frame, app: &App) {
    let vert_layouts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Max(3),
            Constraint::Length(app.height as u16 + 2),
        ])
        .split(frame.area());
    let hor_layouts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(app.width as u16 + 2)])
        .split(vert_layouts[1]);

    let descriptions = Text::from(vec![
        Line::from(format!("横幅({})を縮める/広げる: ←/→", app.width)),
        Line::from(format!("縦幅({})を縮める/広げる: ↑/↓", app.height)),
        Line::from("終了: q"),
    ]);
    frame.render_widget(descriptions, vert_layouts[0]);

    let body = Block::default()
        .border_style(Style::default().fg(Color::White))
        .border_type(BorderType::Rounded)
        .borders(Borders::ALL);
    frame.render_widget(body, hor_layouts[0]);
}
