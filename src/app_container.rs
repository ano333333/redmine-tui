use std::{cell::RefCell, cmp::max, rc::Rc};

use crossterm::event::{Event, KeyCode};
use ratatui::{Frame, layout::Rect};
use std::io::Result;

use crate::{
    app::{Action, Dispatcher},
    components::AppComponent,
};

const APP_INITIAL_WIDTH: u16 = 80;
const APP_INITIAL_HEIGHT: u16 = 80;

const APP_WIDTH_MIN: u16 = 40;
const APP_HEIGHT_MIN: u16 = 40;

pub struct AppContainer {
    width: u16,
    height: u16,
    dispatcher: Rc<RefCell<Dispatcher>>,
    pub app_component: AppComponent,
}

impl AppContainer {
    pub fn new() -> Self {
        let dispatcher = Rc::new(RefCell::new(Dispatcher::new()));
        {
            let mut d = dispatcher.borrow_mut();
            d.dispatch(Action::LoadIssue { id: 1 });
            d.dispatch(Action::LoadIssue { id: 2 });
            d.dispatch(Action::LoadIssue { id: 3 });
            d.dispatch(Action::LoadJournal { id: 1 });
            d.dispatch(Action::LoadJournal { id: 2 });
            d.dispatch(Action::LoadJournal { id: 3 });
            while d.consume_actinos_len() > 0 {
                d.consume_action();
            }
        }
        let mut app_component = AppComponent::new(dispatcher.clone());
        app_component.update(dispatcher.clone(), dispatcher.borrow().store());
        AppContainer {
            width: APP_INITIAL_WIDTH,
            height: APP_INITIAL_HEIGHT,
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
                    // ターミナルからのリサイズイベントに偽装する
                    self.app_component
                        .process_event(Event::Resize(self.width, self.height));
                }
                KeyCode::Right => {
                    self.width = self.width.saturating_add(1);
                    self.app_component
                        .process_event(Event::Resize(self.width, self.height));
                }
                KeyCode::Up => {
                    self.height = max(self.height - 1, APP_HEIGHT_MIN);
                    self.app_component
                        .process_event(Event::Resize(self.width, self.height));
                }
                KeyCode::Down => {
                    self.height = self.height.saturating_add(1);
                    self.app_component
                        .process_event(Event::Resize(self.width, self.height));
                }
                _ => {
                    self.app_component.process_event(event);
                }
            }
        }
        self.app_component
            .update(self.dispatcher.clone(), self.dispatcher.borrow().store());
        true
    }

    pub fn update(&mut self) {
        while self.dispatcher.borrow().consume_actinos_len() > 0 {
            self.dispatcher.borrow_mut().consume_action();
            self.app_component
                .update(self.dispatcher.clone(), self.dispatcher.borrow().store());
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.app_component
            .render(self.dispatcher.borrow().store(), frame, area);
    }

    // AppContainerとターミナル全体のサイズの差を緩衝するメソッド
    // 現在はIssueComponentの初期化時に一回呼ばれるので、それ専用に定数で妥協
    pub fn size() -> Result<(u16, u16)> {
        Ok((APP_INITIAL_WIDTH, APP_INITIAL_HEIGHT))
    }

    pub fn size_of(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}
