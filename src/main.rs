mod app;
mod app_container;
mod components;
mod entities;
mod libs;
mod logging;
#[cfg(test)]
mod test_support;
mod widgets;

use crossterm::event::{self};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders},
};
use std::io::Result;

use self::app_container::AppContainer;

fn main() -> Result<()> {
    logging::initialize_logging()?;
    trace_dbg!("start");
    let mut terminal = ratatui::init();
    let mut app = AppContainer::new();
    loop {
        app.update();
        if let Some(e) = terminal.draw(|f| draw(f, &app)).err() {
            trace_dbg!(level: tracing::Level::ERROR, "failed to draw frame");
            return Err(e);
        }
        match event::read() {
            Ok(event) => {
                if !app.handle_key_event(event, &mut terminal) {
                    break;
                }
            }
            Err(e) => {
                trace_dbg!(level: tracing::Level::ERROR, "failed to read event");
                return Err(e);
            }
        }
    }
    ratatui::restore();
    trace_dbg!("done");
    Ok(())
}

fn draw(frame: &mut Frame, app: &AppContainer) {
    let (width, height) = app.size_of();
    let vert_layouts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(4), Constraint::Length(height + 2)])
        .split(frame.area());
    let hor_layouts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(width + 2)])
        .split(vert_layouts[1]);
    let issue_block = Block::default()
        .border_style(Style::default().fg(Color::White))
        .border_type(BorderType::Rounded)
        .borders(Borders::ALL);
    let issue_area = issue_block.inner(hor_layouts[0]);

    let descriptions = Text::from(vec![
        Line::from(format!("横幅({})を縮める/広げる: ←/→", width)),
        Line::from(format!("縦幅({})を縮める/広げる: ↑/↓", height)),
        Line::from("終了: q"),
    ]);
    frame.render_widget(descriptions, vert_layouts[0]);

    frame.render_widget(issue_block, hor_layouts[0]);

    app.render(frame, issue_area);
}
