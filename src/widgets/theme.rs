//! 画面全体で共有する配色とセクション見出しの定数。
use ratatui::style::Color;

/// フォーカス行の背景色。
///
/// ACCENT(#7AA2F7)と同じ青系で、BADGE_BG(#2A2F41)より一段明るくして
/// フォーカス行が浮いて見えるようにする。
pub const FOCUS_BG: Color = Color::Rgb(0x2E, 0x3A, 0x55);
/// セクション見出しやIDバッジのアクセント色。
pub const ACCENT: Color = Color::Rgb(0x7A, 0xA2, 0xF7);
/// ラベルなど、値より一段落とした前景色。
pub const MUTED: Color = Color::Rgb(0x80, 0x8A, 0x9E);
/// IDバッジなど、反転表示の背景色。
pub const BADGE_BG: Color = Color::Rgb(0x2A, 0x2F, 0x41);
/// Journalのタイムラインを描く線の色。
pub const TIMELINE_FG: Color = Color::Rgb(0x4A, 0x53, 0x66);

/// セクション見出しの左に置くアクセントバー。
pub const SECTION_BAR: &str = "▌";
