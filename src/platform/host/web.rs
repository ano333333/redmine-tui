use std::{
    cell::RefCell,
    collections::VecDeque,
    future::poll_fn,
    io,
    rc::Rc,
    task::{Poll, Waker},
    time::Duration,
};

use futures::{future::Either, future::select};
use ratatui::{Frame, Terminal, layout::Rect};
use ratzilla::{DomBackend, event::KeyEvent as RatzillaKeyEvent, web_sys};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, js_sys::Promise};

use super::{HostEvent, PlatformHost};
use crate::platform::input::{InputEvent, KeyCode, web::convert_key};

#[derive(Default)]
struct InputQueue {
    events: VecDeque<InputEvent>,
    waker: Option<Waker>,
}

pub struct WebPlatformHost {
    terminal: Terminal<DomBackend>,
    started_at: f64,
    input: Rc<RefCell<InputQueue>>,
    key_listener: Closure<dyn FnMut(web_sys::KeyboardEvent)>,
}

impl WebPlatformHost {
    pub fn new(terminal: Terminal<DomBackend>) -> Self {
        let input = Rc::new(RefCell::new(InputQueue::default()));
        let listener_input = Rc::clone(&input);
        let key_listener = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            if let Some(key) = convert_key(RatzillaKeyEvent::from(event.clone())) {
                let InputEvent::Key(key_event) = key;
                // アプリの操作と衝突するキーだけ既定動作を止め、他のブラウザ操作は残す。
                if matches!(key_event.code, KeyCode::Tab | KeyCode::BackTab)
                    || (matches!(key_event.code, KeyCode::Char('s'))
                        && key_event.modifiers.is_control())
                {
                    event.prevent_default();
                }
                // Store と Component の更新は runner に任せ、DOM callback では入力だけ渡す。
                let mut input = listener_input.borrow_mut();
                input.events.push_back(key);
                if let Some(waker) = input.waker.take() {
                    waker.wake();
                }
            }
        }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
        web_sys::window()
            .expect("browser window is unavailable")
            .document()
            .expect("browser document is unavailable")
            .add_event_listener_with_callback("keydown", key_listener.as_ref().unchecked_ref())
            .expect("failed to register browser key listener");
        Self {
            terminal,
            started_at: performance().now(),
            input,
            key_listener,
        }
    }
}

impl Drop for WebPlatformHost {
    fn drop(&mut self) {
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            let _ = document.remove_event_listener_with_callback(
                "keydown",
                self.key_listener.as_ref().unchecked_ref(),
            );
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

    async fn next_event(&mut self, timeout: Duration) -> io::Result<Option<HostEvent>> {
        let input = Rc::clone(&self.input);
        let next_input = poll_fn(|context| {
            let mut input = input.borrow_mut();
            if let Some(event) = input.events.pop_front() {
                Poll::Ready(event)
            } else {
                input.waker = Some(context.waker().clone());
                Poll::Pending
            }
        });
        let timer = std::pin::pin!(sleep(timeout));
        let result = match select(next_input, timer).await {
            Either::Left((event, _)) => Some(HostEvent::Input(event)),
            Either::Right((_, _)) => None,
        };
        self.input.borrow_mut().waker = None;
        Ok(result)
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
