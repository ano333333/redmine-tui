use std::{
    env, fs,
    io::Result,
    path::PathBuf,
    process::{Child, Command, ExitStatus},
    task::Poll,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{EditorOutcome, EditorRequest, TextEditor};

/// 返すFutureはwakerを登録しないため、完了まで定期的にpollするrunnerでのみ使用する。
pub struct NativeTextEditor {
    program: String,
    temp_dir: PathBuf,
}

impl NativeTextEditor {
    pub fn from_environment() -> Self {
        let program = env::var("VISUAL")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| env::var("EDITOR").ok().filter(|value| !value.is_empty()))
            .unwrap_or_else(|| "nvim".to_string());
        Self {
            program,
            temp_dir: env::temp_dir(),
        }
    }

    pub fn with_program(program: impl Into<String>, temp_dir: PathBuf) -> Self {
        Self {
            program: program.into(),
            temp_dir,
        }
    }
}

impl TextEditor for NativeTextEditor {
    fn edit(
        &self,
        request: EditorRequest,
    ) -> impl std::future::Future<Output = Result<EditorOutcome>> {
        let path = editor_path(&self.temp_dir);
        let mut child = start_editor(&self.program, &path, request.initial_text).map_err(Some);
        std::future::poll_fn(move |_| match &mut child {
            Ok(child) => poll_editor(child, &path),
            Err(error) => Poll::Ready(Err(error
                .take()
                .expect("editor future polled after completion"))),
        })
    }
}

fn editor_path(temp_dir: &PathBuf) -> PathBuf {
    let filename = format!(
        "redmine-tui-editor-{}.md",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    temp_dir.join(filename)
}

fn start_editor(program: &str, path: &PathBuf, initial_text: String) -> Result<Child> {
    fs::write(path, initial_text)?;
    Command::new(program).arg(path).spawn()
}

fn poll_editor(child: &mut Child, path: &PathBuf) -> Poll<Result<EditorOutcome>> {
    match child.try_wait() {
        Ok(Some(status)) => {
            if let Err(error) = ensure_editor_exit_status(status) {
                return Poll::Ready(Err(error));
            }
            let edited_text = match fs::read_to_string(path) {
                Ok(edited_text) => edited_text,
                Err(error) => return Poll::Ready(Err(error)),
            };
            let _ = fs::remove_file(path);
            Poll::Ready(Ok(EditorOutcome::Submitted { edited_text }))
        }
        Ok(None) => Poll::Pending,
        Err(error) => Poll::Ready(Err(error)),
    }
}

fn ensure_editor_exit_status(status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "editor exited with non-zero status: {}",
            status.code().map_or_else(
                || "terminated without an exit code".to_string(),
                |code| code.to_string()
            )
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    fn test_temp_dir(test_name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "redmine-tui-editor-{test_name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn await_editor(future: impl Future<Output = Result<EditorOutcome>>) -> Result<EditorOutcome> {
        let mut future = Box::pin(future);
        let deadline = Instant::now() + Duration::from_secs(2);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        loop {
            match Pin::as_mut(&mut future).poll(&mut context) {
                Poll::Ready(outcome) => return outcome,
                Poll::Pending if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10))
                }
                Poll::Pending => panic!("editor did not complete before timeout"),
            }
        }
    }

    #[test]
    fn submits_initial_text_and_removes_the_temporary_file() {
        let temp_dir = test_temp_dir("success");
        let outcome = await_editor(
            NativeTextEditor::with_program("true", temp_dir.clone()).edit(EditorRequest {
                initial_text: "initial text".to_string(),
            }),
        );

        let Ok(EditorOutcome::Submitted { edited_text }) = outcome else {
            panic!("editor did not submit initial text")
        };
        assert_eq!(edited_text, "initial text");
        assert!(fs::read_dir(&temp_dir).unwrap().next().is_none());
        fs::remove_dir(temp_dir).unwrap();
    }

    #[test]
    fn returns_error_when_editor_exits_nonzero() {
        let temp_dir = test_temp_dir("nonzero");
        assert!(
            await_editor(
                NativeTextEditor::with_program("false", temp_dir.clone()).edit(EditorRequest {
                    initial_text: String::new(),
                })
            )
            .is_err()
        );
        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn returns_error_when_editor_cannot_start() {
        let temp_dir = test_temp_dir("start-failure");
        assert!(
            await_editor(
                NativeTextEditor::with_program(
                    "redmine-tui-editor-program-does-not-exist",
                    temp_dir.clone(),
                )
                .edit(EditorRequest {
                    initial_text: String::new(),
                }),
            )
            .is_err()
        );
        fs::remove_dir_all(temp_dir).unwrap();
    }
}
