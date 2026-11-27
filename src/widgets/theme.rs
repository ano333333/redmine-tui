//! 画面全体で共有する配色とセクション見出しの定数。
use ratatui::style::Color;

/// ACCENT(#7AA2F7)と同じ青系で、BADGE_BG(#2A2F41)より一段明るくして
/// フォーカス行が浮いて見えるようにする。
pub const FOCUS_BG: Color = Color::Rgb(0x2E, 0x3A, 0x55);
pub const ACCENT: Color = Color::Rgb(0x7A, 0xA2, 0xF7);
pub const MUTED: Color = Color::Rgb(0x80, 0x8A, 0x9E);
pub const BADGE_BG: Color = Color::Rgb(0x2A, 0x2F, 0x41);
pub const TIMELINE_FG: Color = Color::Rgb(0x4A, 0x53, 0x66);

pub const SECTION_BAR: &str = "▌";
