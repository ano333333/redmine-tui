//! Ratzilla の `DomBackend` のラッパー

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    io,
    rc::Rc,
};

use ratatui::{
    backend::{Backend, ClearType, WindowSize},
    buffer::Cell as BufferCell,
    layout::{Position, Size},
    text::Span,
};
use ratzilla::DomBackend;
use ratzilla::web_sys;
use wasm_bindgen::{JsCast, closure::Closure};

pub(crate) struct WebBackend {
    // Backend::size は &self だが、resize 時に内側の draw を呼ぶ必要がある。
    inner: RefCell<DomBackend>,
    resized: Rc<Cell<bool>>,
    resize_listener: Closure<dyn FnMut(web_sys::Event)>,
    // DomBackend::draw が全角文字の右隣を空文字列にした位置と、
    // 最後に描画したとき全角文字だった位置を別々に記録する。
    // BufferDiff が右隣を差分に含めない場合も、前者を後者と照合して修復できる。
    blanked: RefCell<HashSet<(u16, u16)>>,
    wide: RefCell<HashSet<(u16, u16)>>,
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
            blanked: RefCell::new(HashSet::new()),
            wide: RefCell::new(HashSet::new()),
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
        let content: Vec<_> = content.collect();
        self.inner.get_mut().draw(content.iter().copied())?;

        // Ratzilla は全角文字を描くと右隣の span を ""（幅 0）にする。
        // 後で半角に変わっても ratatui-core の BufferDiff は、buffer 上で値が
        // 変わらない右隣を再描画しないため、DOM の行末が左へずれる。
        let blanked = self.blanked.get_mut();
        let wide = self.wide.get_mut();
        for &(x, y, cell) in &content {
            blanked.remove(&(x, y));
            // DomBackend::draw と同じ全角判定を使う。
            if cell.symbol().len() > 1 && Span::raw(cell.symbol()).width() == 2 {
                wide.insert((x, y));
                if let Some(next_x) = x.checked_add(1) {
                    // 右隣は全角文字の後半として描かれないので、古い全角記録を捨てる。
                    wide.remove(&(next_x, y));
                    blanked.insert((next_x, y));
                }
            } else {
                wide.remove(&(x, y));
            }
        }

        let stale: Vec<_> = blanked
            .iter()
            .copied()
            .filter(|&(x, y)| x == 0 || !wide.contains(&(x - 1, y)))
            .collect();
        // ratatui-core は前の全角文字の背景色などが右隣に見える場合、
        // BufferDiff で右隣を強制的に出し直す。ここに残るのは既定 style の
        // 空白でよいセルだけなので、Cell::default() で幅 0 の span を戻せる。
        let empty = BufferCell::default();
        self.inner
            .get_mut()
            .draw(stale.iter().map(|&(x, y)| (x, y, &empty)))?;
        for position in stale {
            blanked.remove(&position);
        }
        Ok(())
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
            // grid の span はすべて新しくなるため、旧 DOM の幅 0 記録も無効になる。
            self.blanked.borrow_mut().clear();
            self.wide.borrow_mut().clear();
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
