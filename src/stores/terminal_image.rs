//! terminalへの画像表示に使う値を、native版だけが依存する`ratatui-image`から切り離す。

#[cfg(feature = "native")]
use image::DynamicImage;
#[cfg(feature = "native")]
use image::imageops::FilterType;
use ratatui::layout::Size;
#[cfg(feature = "native")]
use ratatui_image::{Resize, picker::Picker, protocol::Protocol};

/// 画像をdecodeするときに確保するメモリの上限。4000x3000px程度の写真（RGBA 8bitで約48 MiB）は扱える。
#[cfg(feature = "native")]
const DECODE_MAX_ALLOC: u64 = 128 * 1024 * 1024;

/// terminalのフォントサイズと対応protocolを持ち、画像を表示用に変換する。
#[cfg(feature = "native")]
#[derive(Clone, Debug)]
pub struct TerminalImageEncoder(Picker);

#[cfg(feature = "native")]
impl TerminalImageEncoder {
    pub fn new(picker: Picker) -> Self {
        Self(picker)
    }

    /// `content`をdecodeし、長辺が`max_side`を超えていれば縮小して、元の解像度で表示する大きさでencodeする。
    pub fn prepare(&self, content: &[u8], max_side: u32) -> Result<PreparedImage, String> {
        // 小さなファイルでも巨大な画像を表すことがあるため、縮小する前のdecodeで確保するメモリに上限を設ける。
        let mut reader = image::ImageReader::new(std::io::Cursor::new(content))
            .with_guessed_format()
            .map_err(|error| error.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(DECODE_MAX_ALLOC);
        reader.limits(limits);
        let image = reader.decode().map_err(|error| error.to_string())?;
        let image = if image.width().max(image.height()) > max_side {
            image.resize(max_side, max_side, FilterType::Triangle)
        } else {
            image
        };
        let font_size = self.0.font_size();
        let size = Size::new(
            cells(image.width(), font_size.width),
            cells(image.height(), font_size.height),
        );
        let protocol = self
            .0
            .new_protocol(image.clone(), size, Resize::Fit(None))
            .map_err(|error| error.to_string())?;
        Ok(PreparedImage::new(image, protocol))
    }
}

#[cfg(feature = "native")]
fn cells(pixels: u32, cell_pixels: u16) -> u16 {
    u16::try_from(pixels.div_ceil(u32::from(cell_pixels))).unwrap_or(u16::MAX)
}

#[cfg(not(feature = "native"))]
impl TerminalImageEncoder {
    pub fn prepare(&self, _: &[u8], _: u32) -> Result<PreparedImage, String> {
        match *self {}
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
    pub(super) fn new(image: DynamicImage, protocol: Protocol) -> Self {
        Self { image, protocol }
    }

    /// 前処理後の画像のピクセル数を`(幅, 高さ)`で返す。
    pub fn pixel_size(&self) -> (u32, u32) {
        (self.image.width(), self.image.height())
    }

    /// 元の解像度で表示したときのセル数を返す。
    pub fn size(&self) -> Size {
        self.protocol.size()
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

#[cfg(not(feature = "native"))]
impl PreparedImage {
    pub fn pixel_size(&self) -> (u32, u32) {
        match *self {}
    }

    pub fn size(&self) -> Size {
        match *self {}
    }
}
