mod app;
mod components;
mod entities;
mod libs;
mod logging;
mod widgets;

use std::{cell::RefCell, cmp::max, rc::Rc};

use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders},
};
use std::io::Result;

use self::app::Dispatcher;
use self::components::{AppComponent, Component};

const APP_WIDTH_MIN: u16 = 40;
const APP_HEIGHT_MIN: u16 = 40;

const APP_COMPONENT_OFFSET_X: u16 = 1;
const APP_COMPONENT_OFFSET_Y: u16 = 5;

struct AppContainer {
    width: u16,
    height: u16,
    dispatcher: Rc<RefCell<Dispatcher>>,
    app_component: AppComponent,
}

impl AppContainer {
    fn new(width: u16, height: u16) -> Self {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut d = dispatcher.borrow_mut();
            d.dispatch(app::Action::LoadIssue { id: 1 });
            d.dispatch(app::Action::LoadIssue { id: 2 });
            d.dispatch(app::Action::LoadIssue { id: 3 });
            d.dispatch(app::Action::LoadJournal { id: 1 });
            d.dispatch(app::Action::LoadJournal { id: 2 });
            d.dispatch(app::Action::LoadJournal { id: 3 });
            d.consume_actions();
        }
        let app_component = AppComponent::new(dispatcher.clone());
        AppContainer {
            width,
            height,
            dispatcher,
            app_component,
        }
    }
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
                _ => {
                    self.app_component.process_event(event);
                }
            }
            true
        } else {
            self.app_component.process_event(event);
            true
        }
    }
    pub fn update(&mut self) {
        self.app_component
            .update(self.dispatcher.clone(), self.dispatcher.borrow().store());
    }
    // AppContainer内とターミナル全体の原点座標の差を緩衝するメソッド
    pub fn set_cursor_position(frame: &mut Frame, position: Position) {
        let x = position.x + APP_COMPONENT_OFFSET_X;
        let y = position.y + APP_COMPONENT_OFFSET_Y;
        frame.set_cursor_position(Position { x, y });
    }
}

fn main() -> Result<()> {
    logging::initialize_logging()?;
    trace_dbg!("start");
    let mut terminal = ratatui::init();
    let mut app = AppContainer::new(80, 80);
    loop {
        app.update();
        if let Some(e) = terminal.draw(|f| draw(f, &app)).err() {
            trace_dbg!(level: tracing::Level::ERROR, "failed to draw frame");
            return Err(e);
        }
        match event::read() {
            Ok(event) => {
                if !app.handle_key_event(event) {
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
    let app_component = &app.app_component;
    let vert_layouts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(4), Constraint::Length(app.height + 2)])
        .split(frame.area());
    let hor_layouts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(app.width + 2)])
        .split(vert_layouts[1]);
    let issue_block = Block::default()
        .border_style(Style::default().fg(Color::White))
        .border_type(BorderType::Rounded)
        .borders(Borders::ALL);
    let issue_area = issue_block.inner(hor_layouts[0]);

    let descriptions = Text::from(vec![
        Line::from(format!("横幅({})を縮める/広げる: ←/→", app.width)),
        Line::from(format!("縦幅({})を縮める/広げる: ↑/↓", app.height)),
        Line::from("終了: q"),
    ]);
    frame.render_widget(descriptions, vert_layouts[0]);

    frame.render_widget(issue_block, hor_layouts[0]);

    app_component.render(app.dispatcher.borrow().store(), frame, issue_area);
}
