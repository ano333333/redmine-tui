//! 本文やnotesのMarkdownを、単独の画像の段落を画像に置き換えてBufferへ描く。

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use super::attachment_image::{AttachmentImageView, resolve_attachment_image};
use crate::entities::Attachment;
use crate::stores::{ImageDisplay, PreparedImage, Store, TerminalImageEncoder};
use crate::widgets::markdown_blocks::{MarkdownBlock, split_image_blocks};

const LOADING_SUFFIX: &str = "（読み込み中）";
const FAILED_SUFFIX: &str = "（読み込めませんでした）";

/// Markdownの画像の参照先を解決するための、Issueの添付ファイルと読み込んだ画像。
pub struct MarkdownImages<'a> {
    source: Option<(&'a Store, &'a [Attachment])>,
}

impl<'a> MarkdownImages<'a> {
    pub fn new(store: &'a Store, attachments: &'a [Attachment]) -> Self {
        Self {
            source: Some((store, attachments)),
        }
    }

    /// 画像を表示しない。画像の段落は代わりの文字で描く。
    #[cfg(test)]
    pub fn none() -> Self {
        Self { source: None }
    }

    fn resolve(&self, dest: &str) -> AttachmentImageView {
        match self.source {
            Some((store, attachments)) => resolve_attachment_image(store, attachments, dest),
            None => AttachmentImageView::Unavailable,
        }
    }

    fn encoder(&self) -> Option<&'a TerminalImageEncoder> {
        match self.source?.0.get_image_display() {
            ImageDisplay::Enabled { encoder } => Some(encoder),
            ImageDisplay::Disabled => None,
        }
    }
}

/// 画像の位置と、表示する幅に合わせた画像。
#[derive(Clone, Debug)]
pub struct MarkdownImagePlacement {
    pub position: Position,
    pub image: PreparedImage,
}

/// Markdownを描いたBufferと、画像を置く位置。画像の行は空けておき、画像は呼び出し側が画面へ直接描く。
pub struct MarkdownView {
    buffer: Buffer,
    images: Vec<MarkdownImagePlacement>,
    /// 幅に合わせて縮小した画像。`update`のたびにencodeし直さないよう、元の画像と幅ごとに残す。
    fitted: Vec<FittedImage>,
}

struct FittedImage {
    source: PreparedImage,
    width: u16,
    fitted: PreparedImage,
}

impl MarkdownView {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::empty(Rect::new(0, 0, 0, 0)),
            images: Vec::new(),
            fitted: Vec::new(),
        }
    }

    pub fn update(&mut self, width: u16, markdown: &str, images: &MarkdownImages) {
        let encoder = images.encoder();
        let mut fitted = Vec::new();
        let mut parts: Vec<Part> = Vec::new();
        for block in split_image_blocks(markdown) {
            let part = match block {
                MarkdownBlock::Text(text) => Part::Text(tui_markdown::from_str(text)),
                MarkdownBlock::Image { dest, source, .. } => match images.resolve(&dest) {
                    AttachmentImageView::Ready(image) => {
                        match encoder
                            .and_then(|encoder| self.fit(encoder, &image, width, &mut fitted))
                        {
                            Some(image) => Part::Image(image),
                            None => Part::Text(fallback(source, Some(FAILED_SUFFIX))),
                        }
                    }
                    AttachmentImageView::Loading => {
                        Part::Text(fallback(source, Some(LOADING_SUFFIX)))
                    }
                    AttachmentImageView::Failed => {
                        Part::Text(fallback(source, Some(FAILED_SUFFIX)))
                    }
                    AttachmentImageView::Unavailable => Part::Text(fallback(source, None)),
                },
            };
            parts.push(part);
        }
        self.fitted = fitted;

        // tui-markdownは段落の間に空行を入れるため、分けたブロックの間にも1行空ける。
        let heights: Vec<u16> = parts.iter().map(|part| part.height(width)).collect();
        let height = heights.iter().sum::<u16>() + (parts.len() as u16).saturating_sub(1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        let mut placements = Vec::new();
        let mut y = 0;
        for (part, part_height) in parts.into_iter().zip(heights) {
            match part {
                Part::Text(text) => {
                    paragraph(text).render(Rect::new(0, y, width, part_height), &mut buffer)
                }
                Part::Image(image) => placements.push(MarkdownImagePlacement {
                    position: Position::new(0, y),
                    image,
                }),
            }
            y += part_height + 1;
        }
        self.buffer = buffer;
        self.images = placements;
    }

    /// 描いたBuffer。画像の行は空いている。
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn line_count(&self) -> u16 {
        self.buffer.area.height
    }

    /// Bufferの左上から見た画像の位置。
    pub fn images(&self) -> &[MarkdownImagePlacement] {
        &self.images
    }

    fn fit(
        &self,
        encoder: &TerminalImageEncoder,
        image: &PreparedImage,
        width: u16,
        fitted: &mut Vec<FittedImage>,
    ) -> Option<PreparedImage> {
        let cached = self
            .fitted
            .iter()
            .chain(fitted.iter())
            .find(|cached| cached.width == width && cached.source.ptr_eq(image))
            .map(|cached| cached.fitted.clone());
        let result = match cached {
            Some(result) => result,
            None => encoder.fit_width(image, width).ok()?,
        };
        if !fitted
            .iter()
            .any(|cached| cached.width == width && cached.source.ptr_eq(image))
        {
            fitted.push(FittedImage {
                source: image.clone(),
                width,
                fitted: result.clone(),
            });
        }
        Some(result)
    }
}

enum Part<'a> {
    Text(Text<'a>),
    Image(PreparedImage),
}

impl Part<'_> {
    fn height(&self, width: u16) -> u16 {
        match self {
            Part::Text(text) => paragraph(text.clone()).line_count(width) as u16,
            Part::Image(image) => image.size().height,
        }
    }
}

fn paragraph(text: Text<'_>) -> Paragraph<'_> {
    Paragraph::new(text).wrap(Wrap { trim: true })
}

/// tui-markdownの`ImageFallback::AltText`の文字に、画像の状態を付ける。
fn fallback<'a>(source: &'a str, suffix: Option<&'static str>) -> Text<'a> {
    let mut text = tui_markdown::from_str(source);
    if let (Some(suffix), Some(line)) = (suffix, text.lines.last_mut()) {
        line.spans.push(Span::from(suffix));
    }
    text
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::layout::Size;

    use super::*;
    use crate::stores::{AttachmentImageAction, ImageAction, ImageKey, SettingsAction};
    use crate::test_support::{sample_attachment, sample_prepared_image, test_image_encoder};
    use crate::vos::AttachmentId;

    const KEY: ImageKey = ImageKey::AttachmentOriginal {
        id: AttachmentId::new(1),
    };

    fn attachments() -> Vec<Attachment> {
        let mut attachment = sample_attachment(1, Some("image/png"), false);
        attachment.filename = "sample.png".to_string();
        vec![attachment]
    }

    fn store_with(actions: Vec<crate::stores::Action>) -> Store {
        let mut store = Store::new();
        store.consume_action(
            SettingsAction::SetImageDisplay(ImageDisplay::Enabled {
                encoder: test_image_encoder(),
            })
            .into(),
        );
        for action in actions {
            store.consume_action(action);
        }
        store
    }

    fn loading_actions() -> Vec<crate::stores::Action> {
        vec![
            AttachmentImageAction::Start {
                attachment_id: AttachmentId::new(1),
                key: KEY,
            }
            .into(),
        ]
    }

    /// テスト用のencoderのフォントは10x20pxのため、`width`x`height`pxは`width / 10`x`height / 20`セルになる。
    fn ready_store(width: u32, height: u32) -> Store {
        let mut actions = loading_actions();
        actions.push(
            ImageAction::Loaded {
                key: KEY,
                path: PathBuf::from("/tmp/image"),
            }
            .into(),
        );
        actions.push(
            ImageAction::Prepared {
                key: KEY,
                image: sample_prepared_image(width, height),
            }
            .into(),
        );
        store_with(actions)
    }

    fn lines(view: &MarkdownView) -> Vec<String> {
        let buffer = view.buffer();
        // 全角文字の2セル目は空白のため、その前のセルが全角文字なら読み飛ばす。
        (0..buffer.area.height)
            .map(|y| {
                let mut line = String::new();
                let mut skip = false;
                for x in 0..buffer.area.width {
                    let symbol = buffer[(x, y)].symbol();
                    if !std::mem::take(&mut skip) {
                        line.push_str(symbol);
                        skip = symbol.chars().any(|c| c >= '\u{1100}');
                    }
                }
                line.trim_end().to_string()
            })
            .collect()
    }

    #[test]
    fn ready_image_leaves_its_rows_empty_between_the_text() {
        let store = ready_store(40, 40);
        let attachments = attachments();
        let mut view = MarkdownView::new();

        view.update(
            20,
            "before\n\n![](sample.png)\n\nafter",
            &MarkdownImages::new(&store, &attachments),
        );

        assert_eq!(lines(&view), vec!["before", "", "", "", "", "after"]);
        let [placement] = view.images() else {
            panic!("expected one image");
        };
        assert_eq!(placement.position, Position::new(0, 2));
        assert_eq!(placement.image.size(), Size::new(4, 2));
    }

    #[test]
    fn image_wider_than_the_width_is_shrunk_once() {
        let store = ready_store(100, 40);
        let attachments = attachments();
        let images = MarkdownImages::new(&store, &attachments);
        let mut view = MarkdownView::new();

        view.update(5, "![](sample.png)", &images);
        let first = view.images()[0].image.clone();
        view.update(5, "![](sample.png)", &images);

        assert_eq!(first.size(), Size::new(5, 1));
        assert!(view.images()[0].image.ptr_eq(&first));
    }

    #[test]
    fn loading_image_is_shown_as_text_with_its_state() {
        let store = store_with(loading_actions());
        let attachments = attachments();
        let mut view = MarkdownView::new();

        view.update(
            40,
            "![](sample.png)",
            &MarkdownImages::new(&store, &attachments),
        );

        assert_eq!(lines(&view), vec!["[img] sample.png（読み込み中）"]);
        assert!(view.images().is_empty());
    }

    #[test]
    fn failed_image_is_shown_as_text_with_its_state() {
        let mut actions = loading_actions();
        actions.push(
            ImageAction::Fail {
                key: KEY,
                reason: "not found".to_string(),
            }
            .into(),
        );
        let store = store_with(actions);
        let attachments = attachments();
        let mut view = MarkdownView::new();

        view.update(
            40,
            "![](sample.png)",
            &MarkdownImages::new(&store, &attachments),
        );

        assert_eq!(
            lines(&view),
            vec!["[img] sample.png（読み込めませんでした）"]
        );
    }

    #[test]
    fn image_without_images_is_shown_as_alt_text() {
        let mut view = MarkdownView::new();

        view.update(40, "![screen](sample.png)", &MarkdownImages::none());

        assert_eq!(lines(&view), vec!["[img] screen"]);
    }
}
