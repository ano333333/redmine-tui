use crate::platform::editor::EditorRequest;
use crate::usecases::UsecaseRequest;

/// Componentが入力の処理中に発行した要求を、runnerが受け取るまで保持する。
///
/// runnerは要求を発行してよい入口にだけ`&mut`で貸す。
#[derive(Default)]
pub struct RequestSink {
    usecases: Vec<UsecaseRequest>,
    editor: Option<EditorRequest>,
}

impl RequestSink {
    pub fn request_usecase(&mut self, request: UsecaseRequest) {
        self.usecases.push(request);
    }

    /// # Panics
    ///
    /// editor sessionは同時に1つしか開けないため、起動要求が既にある場合にpanicする。
    pub fn request_editor(&mut self, request: EditorRequest) {
        assert!(
            self.editor.is_none(),
            "RequestSink already has an editor request"
        );
        self.editor = Some(request);
    }

    pub fn take_usecases(&mut self) -> Vec<UsecaseRequest> {
        std::mem::take(&mut self.usecases)
    }

    pub fn take_editor(&mut self) -> Option<EditorRequest> {
        self.editor.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "RequestSink already has an editor request")]
    fn rejects_a_second_editor_request() {
        let mut sink = RequestSink::default();
        sink.request_editor(EditorRequest {
            initial_text: "first".to_string(),
        });
        sink.request_editor(EditorRequest {
            initial_text: "second".to_string(),
        });
    }
}
