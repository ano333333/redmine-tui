//! 共通runnerから入力、描画、単調時計、editor切り替えを利用するためのplatform境界。
//!
//! runnerはこのtraitだけに依存し、nativeのterminalとWebのDOM backendの違いを扱わない。

use std::{future::Future, io, time::Duration};

use ratatui::{Frame, layout::Rect};

use super::input::InputEvent;

pub mod native;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostEvent {
    Input(InputEvent),
    /// platform eventは発生したが、Componentへ渡す入力ではない。
    ///
    /// timeoutを表す`None`とは区別し、resizeや未対応keyでもrunnerを起床させる。
    Ignored,
}

/// 共通runnerが一つのloopでnative/Webを駆動するために必要なplatform操作。
///
/// editorのFutureはeditorへの参照を完了まで保持するため、editorはhostに所有させず
/// runnerへ別引数で渡す。これにより編集中もhostを可変借用して待機・復帰できる。
pub trait PlatformHost {
    fn area(&mut self) -> Rect;
    /// host生成時を起点とする単調な経過時間を返す。
    fn elapsed(&self) -> Duration;
    fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()>;
    /// event受信時は`Some`、指定時間内にeventがなければ`None`を返す。
    fn next_event(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = io::Result<Option<HostEvent>>>;
    fn wait(&mut self, timeout: Duration) -> impl Future<Output = ()>;
    /// editorへterminalを明け渡す。失敗時は完了済みの操作を戻し、TUIを継続可能にする。
    fn suspend_for_editor(&mut self) -> io::Result<()>;
    /// editor終了後に描画可能な状態へ戻す。途中で失敗しても残りの復帰操作を試みる。
    fn resume_after_editor(&mut self) -> io::Result<()>;
}
