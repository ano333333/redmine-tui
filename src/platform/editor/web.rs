use std::{
    cell::RefCell,
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use ratzilla::web_sys;
use wasm_bindgen::{JsCast, closure::Closure};

use super::{EditorOutcome, EditorRequest, TextEditor};

/// Web editor の入力欄と表示領域を DOM 上で管理する。
struct TextareaOverlay {
    container: web_sys::Element,
    textarea: web_sys::HtmlTextAreaElement,
    // drop すると JS 側の callback が無効になるため、overlay と同じ寿命で保持する。
    _mouse_listener: Closure<dyn FnMut(web_sys::Event)>,
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
        let mouse_textarea = textarea.clone().unchecked_into::<web_sys::EventTarget>();
        let mouse_listener = Closure::wrap(Box::new(move |event: web_sys::Event| {
            // 余白や案内行のクリックで focus が外れると Esc が届かなくなるため、textarea に保つ。
            if event.target().as_ref() != Some(&mouse_textarea) {
                event.prevent_default();
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        container
            .add_event_listener_with_callback("mousedown", mouse_listener.as_ref().unchecked_ref())
            .expect("failed to register editor mouse listener");
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
        // 起動キーの keydown 中に runner が microtask で動くため、ここで focus すると
        // 同じキーの文字入力（例: e）が textarea に入る。setTimeout(0) で次の task に
        // 遅らせ、起動キーの入力が現在の task 内で終わってから focus する。
        let focus_textarea = textarea.clone();
        let focus_callback = Closure::once_into_js(move || {
            // 実行前に overlay が取り除かれている場合があるため、失敗は無視する。
            let _ = focus_textarea.focus();
        });
        web_sys::window()
            .expect("browser window is unavailable")
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                focus_callback.unchecked_ref(),
                0,
            )
            .expect("failed to schedule editor textarea focus");
        Self {
            container,
            textarea,
            _mouse_listener: mouse_listener,
        }
    }

    fn remove(self) {
        if let Some(parent) = self.container.parent_node() {
            let _ = parent.remove_child(&self.container);
        }
    }
}

/// textarea overlay の編集結果を非同期に返す Web editor。
pub struct WebTextEditor;

impl TextEditor for WebTextEditor {
    async fn edit(&self, request: EditorRequest) -> io::Result<EditorOutcome> {
        let overlay = TextareaOverlay::attach(&request.initial_text);
        let completion = EditorCompletion::default();
        let listener_completion = completion.clone();
        let listener_textarea = overlay.textarea.clone();
        let listener = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            // document の listener に届くと、編集中に runner が読まない InputQueue に溜まり、
            // editor 終了後に Component へ流れ込むため、全キーの伝播を止める。
            event.stop_propagation();
            // 変換中の Enter/Esc は IME の確定・取り消しに使うため、preventDefault もせず任せる。
            // Safari などは変換確定キーで is_composing が false になることがあるため、229 も見る。
            if event.is_composing() || event.key_code() == 229 {
                return;
            }
            if event.key() == "Tab" {
                // focus が外れると Esc が届かず、以後のキーも document 経由で queue に溜まる。
                event.prevent_default();
                return;
            }
            let outcome = match event.key().as_str() {
                // 本文を空にする編集も、キャンセルと区別して確定する。
                "Enter" if event.ctrl_key() || event.meta_key() => EditorOutcome::Submitted {
                    edited_text: listener_textarea.value(),
                },
                "Escape" => EditorOutcome::Cancelled,
                _ => return,
            };
            event.prevent_default();
            listener_completion.resolve(outcome);
        }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
        overlay
            .textarea
            .add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
            .expect("failed to register editor key listener");
        let outcome = completion.await;
        overlay
            .textarea
            .remove_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
            .expect("failed to remove editor key listener");
        drop(listener);
        overlay.remove();
        Ok(outcome)
    }
}

/// 複数の DOM イベントから最初の完了だけを受理し、待機側へ渡す。
#[derive(Clone, Default)]
struct EditorCompletion(Rc<RefCell<CompletionState>>);

#[derive(Default)]
struct CompletionState {
    // outcome を取り出した後も、後続イベントによる再完了を拒否する。
    resolved: bool,
    outcome: Option<EditorOutcome>,
    waker: Option<Waker>,
}

impl EditorCompletion {
    fn resolve(&self, outcome: EditorOutcome) {
        let mut state = self.0.borrow_mut();
        if state.resolved {
            return;
        }
        state.resolved = true;
        state.outcome = Some(outcome);
        let waker = state.waker.take();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl Future for EditorCompletion {
    type Output = EditorOutcome;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.0.borrow_mut();
        if let Some(outcome) = state.outcome.take() {
            Poll::Ready(outcome)
        } else {
            state.waker = Some(context.waker().clone());
            Poll::Pending
        }
    }
}
