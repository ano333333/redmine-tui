use std::{cell::RefCell, cmp::max, rc::Rc};

use crossterm::{
    event::{Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{DefaultTerminal, Frame, layout::Rect};
use std::io::Result;
use std::{
    env,
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    app::{Action, Dispatcher},
    components::{
        AppComponent,
        app::{AppEffect, EditorRequest, EditorResponse},
    },
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

    pub fn handle_key_event(&mut self, event: Event, terminal: &mut DefaultTerminal) -> bool {
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
        if let Some(effect) = self.app_component.take_effect() {
            if let Err(err) = self.handle_app_effect(effect, terminal) {
                tracing::event!(
                    target: module_path!(),
                    tracing::Level::ERROR,
                    error = %err,
                    "failed to handle app effect"
                );
            }
        }
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

    fn handle_app_effect(
        &mut self,
        effect: AppEffect,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        match effect {
            AppEffect::OpenEditor(request) => {
                let response = self.run_editor(terminal, request)?;
                self.app_component.handle_editor_response(response);
                self.app_component
                    .update(self.dispatcher.clone(), self.dispatcher.borrow().store());
            }
        }
        Ok(())
    }

    fn run_editor(
        &mut self,
        terminal: &mut DefaultTerminal,
        request: EditorRequest,
    ) -> Result<EditorResponse> {
        let path = Self::create_editor_tmpfile(&request.initial_text)?;
        let editor = env::var("VISUAL")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| env::var("EDITOR").ok().filter(|value| !value.is_empty()))
            .unwrap_or_else(|| "nvim".to_string());

        disable_raw_mode()?;
        execute!(std::io::stdout(), LeaveAlternateScreen)?;
        let status = Command::new(&editor).arg(&path).status();
        let reenter_result = execute!(std::io::stdout(), EnterAlternateScreen);
        let raw_mode_result = enable_raw_mode();
        terminal.clear()?;

        reenter_result?;
        raw_mode_result?;
        status?;

        let edited = fs::read_to_string(&path)?;
        let _ = fs::remove_file(&path);
        Ok(EditorResponse { edited_text: edited })
    }

    fn create_editor_tmpfile(initial_text: &str) -> Result<PathBuf> {
        let filename = format!(
            "redmine-tui-editor-{}.md",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let path = env::temp_dir().join(filename);
        fs::write(&path, initial_text)?;
        Ok(path)
    }
}
