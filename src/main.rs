mod components;
mod logging;

use std::{cmp::max, fs};

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders},
};
use std::io::Result;

use self::components::IssueComponent;
use self::components::{Component, IssueJournalComponent};

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

fn main() -> Result<()> {
    logging::initialize_logging()?;
    trace_dbg!("start");
    let mut terminal = ratatui::init();
    let mut app = App {
        width: 80,
        height: 80,
    };
    loop {
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

fn issue_component() -> IssueComponent {
    fn parse_as_local(str: String) -> DateTime<Local> {
        let naive = NaiveDate::parse_from_str(str.as_str(), "%Y/%m/%d")
            .expect(format!("failed to parse naive datetime: {}", str.as_str()).as_str());
        // Local.from_local_datetime(&naive).single().unwrap()
        let naive_datetime = naive.and_hms_opt(0, 0, 0).unwrap();
        Local.from_local_datetime(&naive_datetime).single().unwrap()
    }
    let body = fs::read_to_string("datas/body.md").expect("failed to read datas/body.md");
    let articles_path = "datas/articles.yml".to_string();
    IssueComponent {
        id: 10000,
        title: "【タスク】Rails 3.2/vendor/plugins非推奨化対応oooooooooooooooooooooooooooooooooooooooooooooooooooo".into(),
        creator: "菊池 雅英".into(),
        appended_at: parse_as_local("2026/02/04".into()),
        updated_at: parse_as_local("2026/02/16".into()),
        status: "進行中(accepted)".to_string(),
        priority: "major".to_string(),
        person_in_charge: Some("菊池 雅英".to_string()),
        target_version: None,
        start_date: Some(parse_as_local("2026/02/16".to_string())),
        due: Some(parse_as_local("2026/02/17".to_string())),
        progress: 0,
        planned_hours: None,
        resolve_way: None,
        component: "IDサーバ".to_string(),
        tags: Vec::<String>::new(),
        body,
        journals: IssueJournalComponent::parse_yaml(&articles_path),
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let vert_layouts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Max(4),
            Constraint::Length(app.height as u16 + 2),
        ])
        .split(frame.area());
    let hor_layouts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(app.width as u16 + 2)])
        .split(vert_layouts[1]);
    let issue_block = Block::default()
        .border_style(Style::default().fg(Color::White))
        .border_type(BorderType::Rounded)
        .borders(Borders::ALL);
    let issue_area = issue_block.inner(hor_layouts[0]);

    let issue_component = issue_component();
    let line_count = issue_component.line_count(issue_area.width);
    let descriptions = Text::from(vec![
        Line::from(format!("横幅({})を縮める/広げる: ←/→", app.width)),
        Line::from(format!("縦幅({})を縮める/広げる: ↑/↓", app.height)),
        Line::from("終了: q"),
        Line::from(format!("全体縦幅: {}", line_count)),
    ]);
    frame.render_widget(descriptions, vert_layouts[0]);

    frame.render_widget(issue_block, hor_layouts[0]);

    issue_component.render(frame, issue_area);
}
