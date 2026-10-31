//! native版の起動時に、設定ファイルの値から画像表示の可否を決める。

use crossterm::terminal::window_size;
use ratatui_image::FontSize;
use ratatui_image::picker::{Picker, ProtocolType};

use crate::stores::{ImageDisplay, TerminalImageEncoder};

/// terminalへは問い合わせず、Kitty graphics protocolに対応したterminalで動いているものとして扱う。
///
/// 画像のセル数を求めるため、terminalのピクセル数からフォントのピクセルサイズを求める。
/// ピクセル数を得られないterminalでは、画像表示を無効にする。
pub(crate) fn image_display_from_config(image_enabled: bool) -> ImageDisplay {
    if !image_enabled {
        return ImageDisplay::Disabled;
    }
    let Some(font_size) = font_size() else {
        tracing::warn!(
            "image display disabled because the terminal does not report its pixel size"
        );
        return ImageDisplay::Disabled;
    };
    // terminalへ問い合わせる`Picker::from_query_stdio`の代わりに、求めたフォントサイズで作る。
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(font_size);
    picker.set_protocol_type(ProtocolType::Kitty);
    tracing::info!(
        width = font_size.width,
        height = font_size.height,
        "image display enabled with the kitty graphics protocol"
    );
    ImageDisplay::Enabled {
        encoder: TerminalImageEncoder::new(picker),
    }
}

fn font_size() -> Option<FontSize> {
    let size = window_size().ok()?;
    if size.width == 0 || size.height == 0 || size.columns == 0 || size.rows == 0 {
        return None;
    }
    let (width, height) = (size.width / size.columns, size.height / size.rows);
    if width == 0 || height == 0 {
        return None;
    }
    Some(FontSize::new(width, height))
}
