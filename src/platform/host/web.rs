use std::{io, time::Duration};

use ratatui::{Frame, Terminal, layout::Rect};
use ratzilla::{DomBackend, web_sys};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, js_sys::Promise};

use super::{HostEvent, PlatformHost};

pub struct WebPlatformHost {
    terminal: Terminal<DomBackend>,
    started_at: f64,
}

impl WebPlatformHost {
    pub fn new(terminal: Terminal<DomBackend>) -> Self {
        Self {
            terminal,
            started_at: performance().now(),
        }
    }
}

impl PlatformHost for WebPlatformHost {
    fn area(&mut self) -> Rect {
        let size = self.terminal.size().expect("failed to get terminal size");
        Rect::new(0, 0, size.width, size.height)
    }

    fn elapsed(&self) -> Duration {
        Duration::from_secs_f64(((performance().now() - self.started_at) / 1000.0).max(0.0))
    }

    fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.terminal.draw(render).map(|_| ())
    }

    // key入力はまだ接続されておらず、timeoutまで待ってrunnerを起床させる。
    async fn next_event(&mut self, timeout: Duration) -> io::Result<Option<HostEvent>> {
        sleep(timeout).await;
        Ok(None)
    }

    async fn wait(&mut self, timeout: Duration) {
        sleep(timeout).await;
    }

    fn suspend_for_editor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn resume_after_editor(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn performance() -> web_sys::Performance {
    web_sys::window()
        .expect("browser window is unavailable")
        .performance()
        .expect("browser performance clock is unavailable")
}

async fn sleep(timeout: Duration) {
    let milliseconds = timeout.as_millis().min(i32::MAX as u128) as i32;
    let promise = Promise::new(&mut |resolve, _reject| {
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        web_sys::window()
            .expect("browser window is unavailable")
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                milliseconds,
            )
            .expect("failed to schedule browser timer");
    });
    JsFuture::from(promise)
        .await
        .expect("browser timer promise was rejected");
}
