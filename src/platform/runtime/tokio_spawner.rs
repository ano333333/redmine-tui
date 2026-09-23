use std::{future::Future, sync::mpsc};

use tokio::{
    runtime::{Builder as TokioRuntimeBuilder, Runtime},
    task::JoinError,
};

use super::{BackgroundCompletion, BackgroundSpawner, panic_message};
use crate::stores::Action;

/// native runner向けにTokio runtimeとbackground taskの完了channelを一体で所有するadapter。
///
/// executorとcompletionの配送経路を同じ境界に閉じ込めることで、呼び出し側へTokio固有型を
/// 渡さずにtaskの起動から完了受理までを扱える。
pub(crate) struct TokioBackgroundSpawner {
    runtime: Runtime,
    completion_receiver: mpsc::Receiver<BackgroundCompletion>,
    completion_sender: mpsc::Sender<BackgroundCompletion>,
}

impl TokioBackgroundSpawner {
    pub(crate) fn new() -> std::io::Result<Self> {
        let runtime = TokioRuntimeBuilder::new_multi_thread()
            .enable_all()
            .build()?;
        let (completion_sender, completion_receiver) = mpsc::channel();
        Ok(Self {
            runtime,
            completion_receiver,
            completion_sender,
        })
    }

    /// event loop開始前の初期読み込みを、background taskと同じruntime上で完了まで進める。
    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

impl BackgroundSpawner for TokioBackgroundSpawner {
    fn spawn<F>(&self, task: F)
    where
        F: Future<Output = Vec<Action>> + Send + 'static,
    {
        let handle = self.runtime.spawn(task);
        let completion_sender = self.completion_sender.clone();
        self.runtime.spawn(async move {
            if let Some(completion) = completion_from_join_result(handle.await) {
                completion_sender
                    .send(completion)
                    .expect("Failed to send BackgroundCompletion with mpsc::channel");
            }
        });
    }

    fn try_recv_completion(&self) -> Option<BackgroundCompletion> {
        self.completion_receiver.try_recv().ok()
    }
}

fn completion_from_join_result(
    result: Result<Vec<Action>, JoinError>,
) -> Option<BackgroundCompletion> {
    match result {
        Ok(actions) => Some(BackgroundCompletion::Succeeded(actions)),
        Err(error) if error.is_panic() => Some(BackgroundCompletion::Panicked {
            message: join_error_panic_message(error),
        }),
        // cancellationはworkerの異常終了ではないため、runnerへ通知を生成しない。
        Err(_) => None,
    }
}

/// Tokioのpanic payloadを共通runtime表現へ渡し、payloadを取得できない場合も共通文言を返す。
fn join_error_panic_message(error: JoinError) -> String {
    let Some(payload) = error.try_into_panic().ok() else {
        return "worker task panicked".to_string();
    };
    panic_message(payload.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{NoticeAction, NoticeId};
    use std::{
        thread,
        time::{Duration, Instant},
    };

    fn recv_completion(spawner: &TokioBackgroundSpawner) -> BackgroundCompletion {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(completion) = spawner.try_recv_completion() {
                return completion;
            }
            assert!(
                Instant::now() < deadline,
                "completion did not arrive in time"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn spawner_returns_successful_actions_in_order() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        spawner.spawn(async {
            vec![
                Action::Notice(NoticeAction::Push {
                    id: NoticeId::new(),
                    message: "first".to_string(),
                }),
                Action::Notice(NoticeAction::Push {
                    id: NoticeId::new(),
                    message: "second".to_string(),
                }),
            ]
        });

        let BackgroundCompletion::Succeeded(actions) = recv_completion(&spawner) else {
            panic!("expected successful completion");
        };
        assert!(matches!(&actions[..], [
            Action::Notice(NoticeAction::Push { message: first, .. }),
            Action::Notice(NoticeAction::Push { message: second, .. }),
        ] if first == "first" && second == "second"));
    }

    #[test]
    fn spawner_returns_panic_message() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        spawner.spawn(async { panic!("worker panic marker") });

        let BackgroundCompletion::Panicked { message } = recv_completion(&spawner) else {
            panic!("expected panic completion");
        };
        assert!(message.contains("worker panic marker"), "got: {message}");
    }

    #[test]
    fn spawner_returns_async_assertion_panic_message() {
        async fn usecase(precondition_met: bool) -> Vec<Action> {
            tokio::task::yield_now().await;
            assert!(precondition_met, "usecase precondition violated");
            Vec::new()
        }

        let spawner = TokioBackgroundSpawner::new().unwrap();
        spawner.spawn(async { usecase(false).await });

        let BackgroundCompletion::Panicked { message } = recv_completion(&spawner) else {
            panic!("expected panic completion");
        };
        assert!(
            message.contains("usecase precondition violated"),
            "got: {message}"
        );
    }

    #[test]
    fn cancelled_task_does_not_produce_a_completion() {
        let spawner = TokioBackgroundSpawner::new().unwrap();
        let handle = spawner.runtime.spawn(std::future::pending::<Vec<Action>>());
        handle.abort();

        let completion = spawner.block_on(async { completion_from_join_result(handle.await) });

        assert!(completion.is_none());
    }
}
