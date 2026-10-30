//! terminalへの画像表示に使う値を、native版だけが依存する`ratatui-image`から切り離す。

#[cfg(feature = "native")]
use std::sync::Arc;

#[cfg(feature = "native")]
use image::DynamicImage;
#[cfg(feature = "native")]
use image::imageops::FilterType;
use ratatui::layout::Size;
#[cfg(feature = "native")]
use ratatui_image::{Resize, picker::Picker, sliced::SlicedProtocol};

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
        self.encode(image)
    }

    /// `image`の幅が`max_width`セルを超えていれば、比率を保って`max_width`セルに収まるよう縮小してencodeし直す。
    ///
    /// 収まる場合は、encodeし直さずに`image`を返す。
    pub fn fit_width(
        &self,
        image: &PreparedImage,
        max_width: u16,
    ) -> Result<PreparedImage, String> {
        if image.size().width <= max_width {
            return Ok(image.clone());
        }
        let max_pixels = u32::from(max_width) * u32::from(self.0.font_size().width);
        let resized = image
            .image
            .resize(max_pixels, image.image.height(), FilterType::Triangle);
        self.encode(resized)
    }

    fn encode(&self, image: DynamicImage) -> Result<PreparedImage, String> {
        let font_size = self.0.font_size();
        let size = Size::new(
            cells(image.width(), font_size.width),
            cells(image.height(), font_size.height),
        );
        let protocol =
            SlicedProtocol::new_with_resize(&self.0, image.clone(), size, Resize::Fit(None))
                .map_err(|error| error.to_string())?;
        Ok(PreparedImage {
            image: Arc::new(image),
            protocol: Arc::new(protocol),
        })
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

    pub fn fit_width(&self, _: &PreparedImage, _: u16) -> Result<PreparedImage, String> {
        match *self {}
    }
}

/// Webデモは画像を表示しないため、値を作れない型にする。
#[cfg(not(feature = "native"))]
#[derive(Clone, Debug)]
pub enum TerminalImageEncoder {}

/// 前処理を済ませ、terminalへ表示できる状態の画像。
///
/// Kittyの送信列は大きいため、cloneしても複製せず共有する。
#[cfg(feature = "native")]
#[derive(Clone)]
pub struct PreparedImage {
    image: Arc<DynamicImage>,
    protocol: Arc<SlicedProtocol>,
}

#[cfg(feature = "native")]
impl PreparedImage {
    /// 前処理後の画像のピクセル数を`(幅, 高さ)`で返す。
    pub fn pixel_size(&self) -> (u32, u32) {
        (self.image.width(), self.image.height())
    }

    /// 表示したときのセル数を返す。
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

#[cfg(test)]
mod tests {
    use ratatui::layout::Size;

    use crate::test_support::{sample_prepared_image, test_image_encoder};

    // テスト用のencoderのフォントは10x20pxのため、100x40pxは10x2セルになる。

    #[test]
    fn image_narrower_than_the_width_is_kept() {
        let image = sample_prepared_image(100, 40);

        let fitted = test_image_encoder().fit_width(&image, 10).unwrap();

        assert_eq!(fitted.size(), Size::new(10, 2));
        assert_eq!(fitted.pixel_size(), (100, 40));
    }

    #[test]
    fn image_wider_than_the_width_is_shrunk_keeping_its_ratio() {
        let image = sample_prepared_image(100, 40);

        let fitted = test_image_encoder().fit_width(&image, 5).unwrap();

        assert_eq!(fitted.pixel_size(), (50, 20));
        assert_eq!(fitted.size(), Size::new(5, 1));
    }
}
