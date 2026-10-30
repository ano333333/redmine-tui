//! Markdownを、単独の画像の段落と、それ以外の文章に分ける。

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// 本文の一部。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownBlock<'a> {
    /// tui-markdownで描く文章。
    Text(&'a str),
    /// 画像1つだけからなる段落。`alt`は説明、`dest`は参照先、`source`は段落の末尾の改行を含む元のMarkdown。
    Image {
        alt: String,
        dest: String,
        source: &'a str,
    },
}

/// tui-markdownと同じ拡張で`markdown`を解析し、最上位の段落のうち画像1つだけからなるものを画像のブロックにする。
///
/// 文中の画像、リストや引用の中の画像、コードブロックの中の記法は文章に残す。空白だけの文章は返さない。
pub fn split_image_blocks(markdown: &str) -> Vec<MarkdownBlock<'_>> {
    let mut blocks = Vec::new();
    let mut text_start = 0;
    for (range, alt, dest) in standalone_images(markdown) {
        push_text(&mut blocks, &markdown[text_start..range.start]);
        blocks.push(MarkdownBlock::Image {
            alt,
            dest,
            source: &markdown[range.clone()],
        });
        text_start = range.end;
    }
    push_text(&mut blocks, &markdown[text_start..]);
    blocks
}

fn push_text<'a>(blocks: &mut Vec<MarkdownBlock<'a>>, text: &'a str) {
    if !text.trim().is_empty() {
        blocks.push(MarkdownBlock::Text(text));
    }
}

/// 最上位の段落のうち、画像1つだけからなるものの範囲、説明、参照先を返す。
fn standalone_images(markdown: &str) -> Vec<(Range<usize>, String, String)> {
    let mut images = Vec::new();
    let mut depth = 0usize;
    let mut paragraph: Option<ParagraphScan> = None;
    for (event, range) in Parser::new_ext(markdown, parse_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Paragraph) if depth == 0 => {
                depth += 1;
                paragraph = Some(ParagraphScan::new(range));
            }
            Event::End(TagEnd::Paragraph) if depth == 1 => {
                depth -= 1;
                if let Some(image) = paragraph.take().and_then(ParagraphScan::into_image) {
                    images.push(image);
                }
            }
            Event::Start(tag) => {
                depth += 1;
                if let Some(scan) = paragraph.as_mut() {
                    scan.start(tag);
                }
            }
            Event::End(tag) => {
                depth -= 1;
                if let Some(scan) = paragraph.as_mut() {
                    scan.end(tag);
                }
            }
            event => {
                if let Some(scan) = paragraph.as_mut() {
                    scan.inline(event);
                }
            }
        }
    }
    images
}

/// 段落の中身が画像1つだけかを、イベントを受け取りながら調べる。
struct ParagraphScan {
    range: Range<usize>,
    image: Option<(String, String)>,
    in_image: bool,
    only_image: bool,
}

impl ParagraphScan {
    fn new(range: Range<usize>) -> Self {
        Self {
            range,
            image: None,
            in_image: false,
            only_image: true,
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Image { dest_url, .. } if self.image.is_none() && !self.in_image => {
                self.in_image = true;
                self.image = Some((String::new(), dest_url.into_string()));
            }
            _ if self.in_image => {}
            _ => self.only_image = false,
        }
    }

    fn end(&mut self, tag: TagEnd) {
        if tag == TagEnd::Image && self.in_image {
            self.in_image = false;
        }
    }

    fn inline(&mut self, event: Event<'_>) {
        match (&mut self.image, self.in_image, event) {
            (Some((alt, _)), true, Event::Text(text) | Event::Code(text)) => alt.push_str(&text),
            (_, true, _) => {}
            (_, false, Event::Text(text)) if text.trim().is_empty() => {}
            _ => self.only_image = false,
        }
    }

    fn into_image(self) -> Option<(Range<usize>, String, String)> {
        let (alt, dest) = self.image?;
        self.only_image.then_some((self.range, alt, dest))
    }
}

/// tui-markdownの`from_str_with_options`と同じ拡張。段落の区切りを同じに解釈するため合わせる。
fn parse_options() -> Options {
    Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
        | Options::ENABLE_MATH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_GFM
        | Options::ENABLE_TABLES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image<'a>(alt: &str, dest: &str, source: &'a str) -> MarkdownBlock<'a> {
        MarkdownBlock::Image {
            alt: alt.to_string(),
            dest: dest.to_string(),
            source,
        }
    }

    #[test]
    fn standalone_image_becomes_an_image_block() {
        assert_eq!(
            split_image_blocks("![](sample.png)"),
            vec![image("", "sample.png", "![](sample.png)")]
        );
    }

    #[test]
    fn text_around_an_image_is_kept_in_order() {
        let markdown = "before\n\n![screen *shot*](a.png)\n\nafter";

        assert_eq!(
            split_image_blocks(markdown),
            vec![
                MarkdownBlock::Text("before\n\n"),
                image("screen shot", "a.png", "![screen *shot*](a.png)\n"),
                MarkdownBlock::Text("\nafter"),
            ]
        );
    }

    #[test]
    fn consecutive_standalone_images_become_separate_blocks() {
        assert_eq!(
            split_image_blocks("![](a.png)\n\n![](b.png)\n"),
            vec![
                image("", "a.png", "![](a.png)\n"),
                image("", "b.png", "![](b.png)\n"),
            ]
        );
    }

    #[test]
    fn image_inside_a_sentence_stays_text() {
        let markdown = "see ![](a.png) here";

        assert_eq!(
            split_image_blocks(markdown),
            vec![MarkdownBlock::Text(markdown)]
        );
    }

    #[test]
    fn two_images_in_one_paragraph_stay_text() {
        let markdown = "![](a.png) ![](b.png)";

        assert_eq!(
            split_image_blocks(markdown),
            vec![MarkdownBlock::Text(markdown)]
        );
    }

    #[test]
    fn image_inside_a_list_stays_text() {
        let markdown = "- ![](a.png)";

        assert_eq!(
            split_image_blocks(markdown),
            vec![MarkdownBlock::Text(markdown)]
        );
    }

    #[test]
    fn image_syntax_inside_a_code_block_stays_text() {
        let markdown = "```\n![](a.png)\n```";

        assert_eq!(
            split_image_blocks(markdown),
            vec![MarkdownBlock::Text(markdown)]
        );
    }

    #[test]
    fn link_wrapping_an_image_stays_text() {
        let markdown = "[![](a.png)](https://example.com)";

        assert_eq!(
            split_image_blocks(markdown),
            vec![MarkdownBlock::Text(markdown)]
        );
    }
}
