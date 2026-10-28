//! terminalへの画像表示に使う値を、native版だけが依存する`ratatui-image`から切り離す。

#[cfg(feature = "native")]
use ratatui_image::picker::Picker;

/// terminalのフォントサイズと対応protocolを持ち、画像を表示用に変換する。
#[cfg(feature = "native")]
#[derive(Clone, Debug)]
pub struct TerminalImageEncoder(Picker);

#[cfg(feature = "native")]
impl TerminalImageEncoder {
    pub fn new(picker: Picker) -> Self {
        Self(picker)
    }
}

/// Webデモは画像を表示しないため、値を作れない型にする。
#[cfg(not(feature = "native"))]
#[derive(Clone, Debug)]
pub enum TerminalImageEncoder {}
