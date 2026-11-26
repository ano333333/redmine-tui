//! 画面に表示する文言を、起動時に選んだ言語のメッセージカタログから引く。

use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::{LanguageIdentifier, langid};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Language {
    #[default]
    English,
    Japanese,
}

impl Language {
    fn identifier(self) -> LanguageIdentifier {
        match self {
            Self::English => langid!("en"),
            Self::Japanese => langid!("ja"),
        }
    }

    fn catalog(self) -> &'static str {
        match self {
            Self::English => include_str!("../locales/en/main.ftl"),
            Self::Japanese => include_str!("../locales/ja/main.ftl"),
        }
    }
}

static BUNDLE: OnceLock<FluentBundle<FluentResource>> = OnceLock::new();

/// 文言を引く前に1度だけ呼ぶ。呼ばなければ英語を使う。
pub(crate) fn set_language(language: Language) {
    BUNDLE
        .set(build_bundle(language))
        .unwrap_or_else(|_| panic!("the language is set once before any message is looked up"));
}

/// カタログにない`id`や、`args`に足りない変数は、カタログとコードの誤りとしてpanicする。
pub(crate) fn message(id: &str, args: Option<&FluentArgs>) -> String {
    let bundle = BUNDLE.get_or_init(|| build_bundle(Language::default()));
    let pattern = bundle
        .get_message(id)
        .and_then(|message| message.value())
        .unwrap_or_else(|| panic!("message `{id}` is missing from the catalog"));
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args, &mut errors);
    assert!(
        errors.is_empty(),
        "failed to format message `{id}`: {errors:?}"
    );
    text.into_owned()
}

/// 文言を、埋め込んだ変数の位置で区切った断片。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Segment {
    Text(String),
    /// `segments`に渡した`names`の何番目の変数か。
    Arg(usize),
}

// カタログの文言には使わない私用領域の文字で、変数の位置に目印を付ける。
const ARG_START: char = '\u{E000}';
const ARG_END: char = '\u{E001}';

/// 変数ごとにスタイルを変えて描くために、`names`の変数の位置で文言を区切る。
pub(crate) fn segments(id: &str, names: &[&str]) -> Vec<Segment> {
    let mut args = FluentArgs::new();
    for (index, name) in names.iter().enumerate() {
        args.set(*name, format!("{ARG_START}{index}{ARG_END}"));
    }
    let text = message(id, Some(&args));
    let mut segments = Vec::new();
    let mut pieces = text.split(ARG_START);
    let mut rest = pieces.next().unwrap_or_default();
    loop {
        if !rest.is_empty() {
            segments.push(Segment::Text(rest.to_string()));
        }
        let Some(piece) = pieces.next() else {
            break;
        };
        let (index, after) = piece
            .split_once(ARG_END)
            .unwrap_or_else(|| broken_marker(id));
        segments.push(Segment::Arg(
            index.parse().unwrap_or_else(|_| broken_marker(id)),
        ));
        rest = after;
    }
    segments
}

fn broken_marker(id: &str) -> ! {
    panic!("message `{id}` has a broken argument marker")
}

/// `tr!("id")`、`tr!("id", name = value, ...)`の形で、`message`に変数を渡して文言を引く。
macro_rules! tr {
    ($id:expr $(,)?) => {
        $crate::i18n::message($id, None)
    };
    ($id:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = ::fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::message($id, Some(&args))
    }};
}
pub(crate) use tr;

fn build_bundle(language: Language) -> FluentBundle<FluentResource> {
    let resource = FluentResource::try_new(language.catalog().to_string())
        .unwrap_or_else(|(_, errors)| panic!("failed to parse the catalog: {errors:?}"));
    let mut bundle = FluentBundle::new_concurrent(vec![language.identifier()]);
    // 埋め込んだ値の前後に双方向分離文字が入ると、terminalでの表示幅が崩れる。
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .unwrap_or_else(|errors| panic!("failed to add the catalog: {errors:?}"));
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tr_embeds_arguments_without_bidi_isolation_marks() {
        let text = tr!("toast-issue-upload-failed", id = 12, reason = "timeout");

        assert_eq!(text, "Failed to save issue #12: timeout");
    }

    #[test]
    fn segments_split_the_message_at_each_argument() {
        assert_eq!(
            segments("journal-detail-attr-changed", &["property", "old", "new"]),
            [
                Segment::Arg(0),
                Segment::Text(" changed from ".to_string()),
                Segment::Arg(1),
                Segment::Text(" to ".to_string()),
                Segment::Arg(2),
            ]
        );
    }
}
