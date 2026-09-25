use std::io;

use super::{EditorOutcome, EditorRequest, TextEditor};

/// Webでの編集UIが接続されるまで、編集要求をUnsupportedとして返す暫定実装。
pub struct WebTextEditor;

impl TextEditor for WebTextEditor {
    async fn edit(&self, _request: EditorRequest) -> io::Result<EditorOutcome> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "web editor is not available yet",
        ))
    }
}
