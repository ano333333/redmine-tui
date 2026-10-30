//! terminalへの画像表示に使う値を、native版だけが依存する`ratatui-image`から切り離す。

#[cfg(feature = "native")]
use image::DynamicImage;
#[cfg(feature = "native")]
use ratatui_image::{picker::Picker, protocol::Protocol};

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

/// 前処理を済ませ、terminalへ表示できる状態の画像。
#[cfg(feature = "native")]
#[derive(Clone)]
pub struct PreparedImage {
    image: DynamicImage,
    protocol: Protocol,
}

#[cfg(feature = "native")]
impl PreparedImage {
    /// `protocol`は、`image`を元の解像度で表示する大きさでencodeしたものを渡す。
    pub fn new(image: DynamicImage, protocol: Protocol) -> Self {
        Self { image, protocol }
    }
}

#[cfg(feature = "native")]
impl std::fmt::Debug for PreparedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // encode済みのデータは大きいため、大きさだけを出す。
        f.debug_struct("PreparedImage")
            .field("width", &self.image.width())
            .field("height", &self.image.height())
            .field("size", &self.protocol.size())
            .finish()
    }
}

#[cfg(not(feature = "native"))]
#[derive(Clone, Debug)]
pub enum PreparedImage {}
