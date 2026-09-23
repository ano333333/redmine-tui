#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    Application,
    Editing,
}

pub struct EditorRequest {
    pub initial_text: String,
}

pub enum EditorOutcome {
    Submitted {
        edited_text: String,
    },
    /// Web editorなど、明示的なキャンセル操作を持つeditorが返す。native editorは常に`Submitted`を返す。
    Cancelled,
    /// editor error時にcontextを解放するための完了通知。元のerrorは呼び出し側が別途伝播する。
    Failed,
}

/// platform固有のeditor sessionを開始し、同じUI thread上で完了を待つ。
///
/// Web実装はDOM要素やJS callbackなど`!Send`な値を保持したまま完了を待つため、返すFutureに`Send`を要求しない。
pub trait TextEditor {
    fn edit(&self, request: EditorRequest) -> impl std::future::Future<Output = EditorOutcome>;
}
