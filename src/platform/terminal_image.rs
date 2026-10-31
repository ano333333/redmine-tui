//! native版の起動時に、設定ファイルの値から画像表示の可否を決める。

use crossterm::terminal::window_size;
use ratatui_image::FontSize;
use ratatui_image::picker::{Picker, ProtocolType};

use crate::stores::{ImageDisplay, TerminalImageEncoder};

/// terminalへは問い合わせず、Kitty graphics protocolに対応したterminalで動いているものとして扱う。
///
/// 画像のセル数を求めるため、terminalのピクセル数からフォントのピクセルサイズを求める。
/// 画像表示が有効なのにピクセル数を得られない場合は、利用者へ伝える理由を返す。
pub(crate) fn image_display_from_config(image_enabled: bool) -> Result<ImageDisplay, String> {
    if !image_enabled {
        return Ok(ImageDisplay::Disabled);
    }
    let font_size = font_size()?;
    // terminalへ問い合わせる`Picker::from_query_stdio`の代わりに、求めたフォントサイズで作る。
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(font_size);
    picker.set_protocol_type(ProtocolType::Kitty);
    tracing::info!(
        width = font_size.width,
        height = font_size.height,
        "image display enabled with the kitty graphics protocol"
    );
    Ok(ImageDisplay::Enabled {
        encoder: TerminalImageEncoder::new(picker),
    })
}

fn font_size() -> Result<FontSize, String> {
    let size = window_size().map_err(|error| error.to_string())?;
    if size.width == 0 || size.height == 0 || size.columns == 0 || size.rows == 0 {
        return Err("the terminal does not report its size in pixels".to_string());
    }
    let (width, height) = (size.width / size.columns, size.height / size.rows);
    if width == 0 || height == 0 {
        return Err("the terminal reports an invalid size in pixels".to_string());
    }
    Ok(FontSize::new(width, height))
}
