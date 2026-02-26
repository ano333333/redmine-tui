use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::{Action, Dispatcher, Store};
use crate::components::issue::IssueComponent;
use crate::components::{Component, JournalComponent, RelativeIssueComponent};

pub struct AppComponent {
    issue_component: IssueComponent,
    dispatcher: Rc<RefCell<Dispatcher>>,
}

impl AppComponent {
    pub fn new(dispatcher: Rc<RefCell<Dispatcher>>) -> Self {
        fn parse_as_local(str: String) -> DateTime<Local> {
            let naive = NaiveDate::parse_from_str(str.as_str(), "%Y/%m/%d")
                .expect(format!("failed to parse naive datetime: {}", str.as_str()).as_str());
            // Local.from_local_datetime(&naive).single().unwrap()
            let naive_datetime = naive.and_hms_opt(0, 0, 0).unwrap();
            Local.from_local_datetime(&naive_datetime).single().unwrap()
        }
        let body = fs::read_to_string("datas/body.md").expect("failed to read datas/body.md");
        let relatives_path = "datas/relatives.yml".to_string();
        let articles_path = "datas/articles.yml".to_string();
        AppComponent {
            issue_component: IssueComponent {
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
                relatives: RelativeIssueComponent::parse_yaml(&relatives_path),
                journals: JournalComponent::parse_yaml(&articles_path),
            },
            dispatcher,
        }
    }

    pub fn process_event(&self, event: Event) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('p') => {
                    let mut d = self.dispatcher.borrow_mut();
                    d.dispatch(Action::Increment);
                }
                _ => {}
            }
        }
    }
}

impl Component for AppComponent {
    fn line_count(&self, width: u16) -> u16 {
        self.issue_component.line_count(width)
    }

    fn render(&self, store: &Store, frame: &mut Frame, mut area: Rect) {
        self.issue_component.render(store, frame, area);
    }
}
