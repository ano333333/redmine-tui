//! native版の起動時に、terminalが画像を表示できるかを問い合わせる。

use ratatui_image::picker::{Picker, ProtocolType};

use crate::stores::{ImageDisplay, TerminalImageEncoder};

/// alternate screenに入った後、入力イベントを読み始める前に呼ぶ。
///
/// 利用者が画像表示を有効にしていなければ、terminalへ問い合わせない。
pub(crate) fn detect_image_display(image_enabled: bool) -> ImageDisplay {
    if !image_enabled {
        return ImageDisplay::Disabled;
    }
    match Picker::from_query_stdio() {
        Ok(picker) if picker.protocol_type() == ProtocolType::Kitty => {
            tracing::info!("image display enabled with the kitty graphics protocol");
            ImageDisplay::Enabled {
                encoder: TerminalImageEncoder::new(picker),
            }
        }
        Ok(picker) => {
            tracing::info!(
                protocol = ?picker.protocol_type(),
                "image display disabled because the terminal does not support the kitty graphics protocol"
            );
            ImageDisplay::Disabled
        }
        Err(error) => {
            tracing::warn!(%error, "image display disabled because the terminal query failed");
            ImageDisplay::Disabled
        }
    }
}
