use std::io;

use ratzilla::web_sys;
use wasm_bindgen::JsCast;

use super::{EditorOutcome, EditorRequest, TextEditor};

/// Web editor の入力欄と表示領域を DOM 上で管理する。
struct TextareaOverlay {
    container: web_sys::Element,
    textarea: web_sys::HtmlTextAreaElement,
}

impl TextareaOverlay {
    fn attach(initial_text: &str) -> Self {
        let document = web_sys::window()
            .expect("browser window is unavailable")
            .document()
            .expect("browser document is unavailable");
        let container = document
            .create_element("div")
            .expect("failed to create editor overlay");
        container.set_class_name("editor-overlay");
        let textarea = document
            .create_element("textarea")
            .expect("failed to create editor textarea")
            .dyn_into::<web_sys::HtmlTextAreaElement>()
            .expect("created textarea has an unexpected element type");
        textarea.set_class_name("editor-overlay__textarea");
        textarea.set_value(initial_text);
        container
            .append_child(&textarea)
            .expect("failed to add textarea to editor overlay");
        let hint = document
            .create_element("div")
            .expect("failed to create editor key hint");
        hint.set_class_name("editor-overlay__hint");
        hint.set_text_content(Some("Submit: Ctrl+Enter · Cancel: Esc"));
        container
            .append_child(&hint)
            .expect("failed to add key hint to editor overlay");
        document
            .body()
            .expect("browser document has no body")
            .append_child(&container)
            .expect("failed to attach editor overlay");
        textarea.focus().expect("failed to focus editor textarea");
        Self {
            container,
            textarea,
        }
    }

    fn value(&self) -> String {
        self.textarea.value()
    }

    fn remove(self) {
        if let Some(parent) = self.container.parent_node() {
            let _ = parent.remove_child(&self.container);
        }
    }
}

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
