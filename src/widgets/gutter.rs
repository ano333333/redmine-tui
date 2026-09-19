//! ブロックの左端に縦線を引き、本文を字下げして見せるためのヘルパー。
//!
//! 属性・説明・子チケット・Journal がそれぞれ同じ字下げ幅で並ぶよう、
//! 線の見た目と幅をここに集約する。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::theme::TIMELINE_FG;

/// 縦線を描く左端の幅。`● ` / `│ ` の2桁。
pub const GUTTER_WIDTH: u16 = 2;
/// ブロックの先頭に置くマーカー。Journalのように起点を示したいときに使う。
pub const GUTTER_HEAD: &str = "●";
/// ブロックを繋ぐ縦線。
pub const GUTTER_LINE: &str = "│";

/// 縦線の引き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterStyle {
    /// 全行を縦線で繋ぐ。属性や説明のような、起点を持たないブロック向け。
    Line,
    /// 先頭行だけ `●`、以降を縦線で繋ぐ。Journalのエントリ向け。
    HeadedLine,
}

pub struct Gutter {
    style: GutterStyle,
}

impl Gutter {
    pub fn line() -> Self {
        Self {
            style: GutterStyle::Line,
        }
    }

    pub fn headed_line() -> Self {
        Self {
            style: GutterStyle::HeadedLine,
        }
    }
}

impl Widget for Gutter {
    /// `area` の左端1列に縦線を描く。
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in 0..area.height {
            let symbol = match (self.style, y) {
                (GutterStyle::HeadedLine, 0) => GUTTER_HEAD,
                _ => GUTTER_LINE,
            };

            if let Some(cell) = buf.cell_mut((area.x, area.y + y)) {
                cell.set_symbol(symbol);
                cell.set_fg(TIMELINE_FG);
            }
        }
    }
}

/// 縦線の分だけ右へ寄せた、本文用の描画領域を返す。
pub fn indented_area(area: Rect) -> Rect {
    Rect::new(
        area.x + GUTTER_WIDTH,
        area.y,
        area.width.saturating_sub(GUTTER_WIDTH),
        area.height,
    )
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;

    #[test]
    fn line_renders_through_the_last_row_only_in_the_left_column() {
        let area = Rect::new(0, 0, 3, 3);
        let mut buffer = Buffer::empty(area);

        Gutter::line().render(area, &mut buffer);

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), GUTTER_LINE);
        assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), GUTTER_LINE);
        assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), GUTTER_LINE);
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((2, 1)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, TIMELINE_FG);
        assert_eq!(buffer.cell((1, 0)).unwrap().fg, Color::Reset);
    }

    #[test]
    fn headed_line_renders_a_head_only_on_the_first_row() {
        let area = Rect::new(0, 0, 1, 3);
        let mut buffer = Buffer::empty(area);

        Gutter::headed_line().render(area, &mut buffer);

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), GUTTER_HEAD);
        assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), GUTTER_LINE);
        assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), GUTTER_LINE);
    }
}
