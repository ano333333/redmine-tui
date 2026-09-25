//! Ratzilla の DOM 再構築を ratatui のサイズ取得より先に進める Backend wrapper。
//! resize 後も Terminal が最新の大きさで描画 buffer を作れるようにする。

use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};

use ratatui::{
    backend::{Backend, ClearType, WindowSize},
    buffer::Cell as BufferCell,
    layout::{Position, Size},
};
use ratzilla::DomBackend;
use ratzilla::web_sys;
use wasm_bindgen::{JsCast, closure::Closure};

pub(crate) struct WebBackend {
    // Backend::size は &self だが、resize 時に内側の draw を呼ぶ必要がある。
    inner: RefCell<DomBackend>,
    resized: Rc<Cell<bool>>,
    resize_listener: Closure<dyn FnMut(web_sys::Event)>,
}

impl WebBackend {
    pub(crate) fn new(inner: DomBackend) -> Self {
        let resized = Rc::new(Cell::new(false));
        let listener_resized = Rc::clone(&resized);
        let resize_listener = Closure::wrap(Box::new(move |_: web_sys::Event| {
            listener_resized.set(true);
        }) as Box<dyn FnMut(web_sys::Event)>);
        // Ratzilla の DomBackend::add_on_resize_listener は window.set_onresize を使う。
        // 同じ setter を使うと grid 再構築のための Ratzilla 側 handler を上書きする。
        web_sys::window()
            .expect("browser window is unavailable")
            .add_event_listener_with_callback("resize", resize_listener.as_ref().unchecked_ref())
            .expect("failed to register backend resize listener");
        Self {
            inner: RefCell::new(inner),
            resized,
            resize_listener,
        }
    }
}

impl Drop for WebBackend {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                "resize",
                self.resize_listener.as_ref().unchecked_ref(),
            );
        }
    }
}

impl Backend for WebBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a BufferCell)>,
    {
        self.inner.get_mut().draw(content)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.inner.get_mut().append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.get_mut().hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.get_mut().show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.get_mut().get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.inner.get_mut().set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.inner.get_mut().clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.inner.get_mut().clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        if self.resized.get() {
            // Ratzilla の resize callback は initialized を false にするだけで、
            // grid（div#grid と pre/span）と size は次の DomBackend::draw で更新する。
            // Terminal::draw は描画前の size で buffer を作るため、古い size のままだと
            // 新しい grid に対する差分座標がずれ、縮小時には範囲外で panic しうる。
            // 空の draw で先に再構築すれば、autoresize が新しい size を検知して
            // buffer を作り直し、空の grid 全体を描画できる。
            self.inner.borrow_mut().draw(std::iter::empty())?;
            self.resized.set(false);
        }
        self.inner.borrow().size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.get_mut().window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.get_mut().flush()
    }
}
