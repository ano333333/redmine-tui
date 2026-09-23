use std::{
    io,
    sync::atomic::{self, AtomicBool},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{DefaultTerminal, Frame, layout::Rect};

use super::{HostEvent, PlatformHost};
use crate::platform::input::native::convert_key;

static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

pub struct NativePlatformHost {
    terminal: DefaultTerminal,
    // Noticeの表示時間をwall clockの補正から独立させるため、経過時間は単調時計で測る。
    started_at: Instant,
}

impl NativePlatformHost {
    pub fn new(terminal: DefaultTerminal) -> Self {
        Self {
            terminal,
            started_at: Instant::now(),
        }
    }
}

impl PlatformHost for NativePlatformHost {
    fn area(&mut self) -> Rect {
        let size = self.terminal.size().expect("failed to get terminal size");
        area_from_terminal_size(size.width, size.height)
    }

    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.terminal.draw(render).map(|_| ())
    }

    async fn next_event(&mut self, timeout: Duration) -> io::Result<Option<HostEvent>> {
        if !event::poll(timeout)? {
            return Ok(None);
        }
        Ok(Some(host_event(event::read()?)))
    }

    async fn wait(&mut self, timeout: Duration) {
        // nativeでは同期的に待ってもbrowser event loopを塞ぐ制約がないため、専用timerを持たない。
        std::thread::sleep(timeout);
    }

    fn suspend_for_editor(&mut self) -> io::Result<()> {
        let mut disable_raw = || disable_raw_mode();
        let mut leave_alternate_screen = || execute!(std::io::stdout(), LeaveAlternateScreen);
        let mut enable_raw = || enable_raw_mode();
        let mut enter_alternate_screen = || execute!(std::io::stdout(), EnterAlternateScreen);
        run_terminal_operations_with_rollback(&mut [
            (&mut disable_raw, &mut enable_raw),
            (&mut leave_alternate_screen, &mut enter_alternate_screen),
        ])
    }

    fn resume_after_editor(&mut self) -> io::Result<()> {
        let mut enter_alternate_screen = || execute!(std::io::stdout(), EnterAlternateScreen);
        let mut enable_raw = || enable_raw_mode();
        let mut clear_terminal = || self.terminal.clear();
        run_terminal_operations(&mut [
            &mut enter_alternate_screen,
            &mut enable_raw,
            &mut clear_terminal,
        ])
    }
}

fn host_event(event: Event) -> HostEvent {
    match event {
        Event::Key(key) => convert_key(key).map_or(HostEvent::Ignored, HostEvent::Input),
        _ => HostEvent::Ignored,
    }
}

fn area_from_terminal_size(width: u16, height: u16) -> Rect {
    Rect::new(0, 0, width, height)
}

fn run_terminal_operations(
    operations: &mut [&mut dyn FnMut() -> io::Result<()>],
) -> io::Result<()> {
    // 途中の失敗後もraw modeを戻せるよう、後続のterminal復帰操作はすべて試みる。
    let mut first_error = None;
    for operation in operations {
        if let Err(error) = operation() {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn run_terminal_operations_with_rollback(
    operations: &mut [(
        &mut dyn FnMut() -> io::Result<()>,
        &mut dyn FnMut() -> io::Result<()>,
    )],
) -> io::Result<()> {
    let mut completed = 0;
    for (operation, _) in operations.iter_mut() {
        if let Err(error) = operation() {
            // 中途状態のterminalをTUIで操作可能な状態へ戻すため、完了済みの操作だけを逆順に戻す。
            for (_, rollback) in operations[..completed].iter_mut().rev() {
                let _ = rollback();
            }
            return Err(error);
        }
        completed += 1;
    }
    Ok(())
}

/// panic 時にも端末を raw mode のまま残さず、既定の hook による報告は維持する。
pub(crate) fn install_panic_hook() {
    // hook を重ねると、以前の custom hook を default_hook として再度呼び出してしまう。
    if PANIC_HOOK_INSTALLED.swap(true, atomic::Ordering::SeqCst) {
        return;
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::platform::input::{InputEvent, KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn area_from_terminal_size_uses_the_latest_dimensions() {
        assert_eq!(area_from_terminal_size(120, 40), Rect::new(0, 0, 120, 40));
    }

    #[test]
    fn host_event_converts_or_ignores_crossterm_events() {
        assert_eq!(
            host_event(Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('j'),
                crossterm::event::KeyModifiers::NONE,
            ))),
            HostEvent::Input(InputEvent::Key(KeyEvent::new(
                KeyCode::Char('j'),
                KeyModifiers::none(),
            )),)
        );
        assert_eq!(
            host_event(Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('x'),
                crossterm::event::KeyModifiers::ALT,
            ))),
            HostEvent::Ignored
        );
        assert_eq!(host_event(Event::Resize(120, 40)), HostEvent::Ignored);
    }

    #[test]
    fn terminal_operations_run_every_operation_and_return_the_first_error() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let first_calls = calls.clone();
        let mut first = move || {
            first_calls.borrow_mut().push("first");
            Err(io::Error::other("first failure"))
        };
        let second_calls = calls.clone();
        let mut second = move || {
            second_calls.borrow_mut().push("second");
            Err(io::Error::other("second failure"))
        };
        let third_calls = calls.clone();
        let mut third = move || {
            third_calls.borrow_mut().push("third");
            Ok(())
        };

        let error = run_terminal_operations(&mut [&mut first, &mut second, &mut third])
            .expect_err("the first operation should fail");

        assert_eq!(error.to_string(), "first failure");
        assert_eq!(*calls.borrow(), ["first", "second", "third"]);
    }

    #[test]
    fn terminal_operations_succeed_when_every_operation_succeeds() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let first_calls = calls.clone();
        let mut first = move || {
            first_calls.borrow_mut().push("first");
            Ok(())
        };
        let second_calls = calls.clone();
        let mut second = move || {
            second_calls.borrow_mut().push("second");
            Ok(())
        };

        assert!(run_terminal_operations(&mut [&mut first, &mut second]).is_ok());
        assert_eq!(*calls.borrow(), ["first", "second"]);
    }

    #[test]
    fn terminal_operations_rollback_only_completed_operations_after_a_failure() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let first_calls = calls.clone();
        let mut first = move || {
            first_calls.borrow_mut().push("first");
            Ok(())
        };
        let first_rollback_calls = calls.clone();
        let mut first_rollback = move || {
            first_rollback_calls.borrow_mut().push("first rollback");
            Ok(())
        };
        let second_calls = calls.clone();
        let mut second = move || {
            second_calls.borrow_mut().push("second");
            Err(io::Error::other("second failure"))
        };
        let second_rollback_calls = calls.clone();
        let mut second_rollback = move || {
            second_rollback_calls.borrow_mut().push("second rollback");
            Ok(())
        };

        let error = run_terminal_operations_with_rollback(&mut [
            (&mut first, &mut first_rollback),
            (&mut second, &mut second_rollback),
        ])
        .expect_err("the second operation should fail");

        assert_eq!(error.to_string(), "second failure");
        assert_eq!(*calls.borrow(), ["first", "second", "first rollback"]);
    }

    #[test]
    fn terminal_operations_do_not_rollback_after_all_operations_succeed() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let operation_calls = calls.clone();
        let mut operation = move || {
            operation_calls.borrow_mut().push("operation");
            Ok(())
        };
        let rollback_calls = calls.clone();
        let mut rollback = move || {
            rollback_calls.borrow_mut().push("rollback");
            Ok(())
        };

        assert!(
            run_terminal_operations_with_rollback(&mut [(&mut operation, &mut rollback)]).is_ok()
        );
        assert_eq!(*calls.borrow(), ["operation"]);
    }
}
