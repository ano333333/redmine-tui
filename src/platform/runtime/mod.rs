use std::any::Any;
use std::future::Future;

use crate::stores::Action;

pub(crate) mod tokio_spawner;

/// background taskからrunnerへ返す、実行環境に依存しない完了通知。
///
/// task側ではStoreを更新せず、runnerがこの通知をUI thread上でAction dispatchへ接続する。
pub enum BackgroundCompletion {
    Succeeded(Vec<Action>),
    Panicked { message: String },
}

/// background taskの起動方法と完了通知の配送を、executor固有の型から分離するport。
pub trait BackgroundSpawner {
    /// Redmine I/Oなど、UI threadから独立して進められるtaskを起動する。
    ///
    /// Web adapterでも同じbackground task契約を保つため、Futureの`Send + 'static`は緩めない。
    fn spawn<F>(&self, task: F)
    where
        F: Future<Output = Vec<Action>> + Send + 'static;

    /// 完了済みの通知を受理順に1件だけ取り出し、未完了なら直ちに`None`を返す。
    fn try_recv_completion(&self) -> Option<BackgroundCompletion>;
}

/// 通常の文字列panic payloadをmessageへ変換し、それ以外の型には共通文言を返す。
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload.downcast_ref::<&str>().map_or_else(
        || {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "worker task panicked".to_string())
        },
        |message| message.to_string(),
    )
}

impl From<Vec<Action>> for BackgroundCompletion {
    fn from(actions: Vec<Action>) -> Self {
        Self::Succeeded(actions)
    }
}

impl From<Box<dyn Any + Send>> for BackgroundCompletion {
    fn from(payload: Box<dyn Any + Send>) -> Self {
        Self::Panicked {
            message: panic_message(payload.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Poll, Waker};

    struct FakeSpawner {
        tasks: RefCell<Vec<Pin<Box<dyn Future<Output = Vec<Action>> + Send>>>>,
        completions: RefCell<VecDeque<BackgroundCompletion>>,
    }

    impl FakeSpawner {
        fn poll_tasks(&self) {
            let mut tasks = std::mem::take(&mut *self.tasks.borrow_mut());
            let mut pending = Vec::new();
            let mut context = Context::from_waker(Waker::noop());
            for mut task in tasks.drain(..) {
                match task.as_mut().poll(&mut context) {
                    Poll::Ready(actions) => self
                        .completions
                        .borrow_mut()
                        .push_back(BackgroundCompletion::Succeeded(actions)),
                    Poll::Pending => pending.push(task),
                }
            }
            *self.tasks.borrow_mut() = pending;
        }
    }

    impl BackgroundSpawner for FakeSpawner {
        fn spawn<F>(&self, task: F)
        where
            F: Future<Output = Vec<Action>> + Send + 'static,
        {
            self.tasks.borrow_mut().push(Box::pin(task));
            self.poll_tasks();
        }

        fn try_recv_completion(&self) -> Option<BackgroundCompletion> {
            self.completions.borrow_mut().pop_front()
        }
    }

    struct ControlledTask {
        started: Arc<AtomicBool>,
        ready: Arc<AtomicBool>,
        actions: Vec<Action>,
    }

    impl Future for ControlledTask {
        type Output = Vec<Action>;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            let task = self.get_mut();
            task.started.store(true, Ordering::SeqCst);
            task.ready
                .load(Ordering::SeqCst)
                .then(|| std::mem::take(&mut task.actions))
                .map_or(Poll::Pending, Poll::Ready)
        }
    }

    #[test]
    fn maps_success_value_to_succeeded() {
        let completion = BackgroundCompletion::from(vec![
            Action::WorkerPanicked {
                message: "first".to_string(),
            },
            Action::WorkerPanicked {
                message: "second".to_string(),
            },
        ]);
        let BackgroundCompletion::Succeeded(actions) = completion else {
            panic!("expected successful completion")
        };
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], Action::WorkerPanicked { message } if message == "first"));
        assert!(matches!(&actions[1], Action::WorkerPanicked { message } if message == "second"));
    }

    #[test]
    fn maps_panic_payloads_to_panicked() {
        let cases: Vec<(Box<dyn Any + Send>, &str)> = vec![
            (Box::new("worker panic"), "worker panic"),
            (Box::new("worker panic".to_string()), "worker panic"),
            (Box::new(42_u8), "worker task panicked"),
        ];
        for (payload, expected_message) in cases {
            assert!(matches!(
                BackgroundCompletion::from(payload),
                BackgroundCompletion::Panicked { message } if message == expected_message
            ));
        }
    }

    #[test]
    fn spawner_starts_tasks_and_returns_completions_in_acceptance_order() {
        let spawner = FakeSpawner {
            tasks: RefCell::default(),
            completions: RefCell::default(),
        };
        let first_started = Arc::new(AtomicBool::new(false));
        let first_ready = Arc::new(AtomicBool::new(false));
        let second_started = Arc::new(AtomicBool::new(false));
        let second_ready = Arc::new(AtomicBool::new(false));

        spawner.spawn(ControlledTask {
            started: Arc::clone(&first_started),
            ready: Arc::clone(&first_ready),
            actions: vec![Action::WorkerPanicked {
                message: "first".to_string(),
            }],
        });
        spawner.spawn(ControlledTask {
            started: Arc::clone(&second_started),
            ready: Arc::clone(&second_ready),
            actions: vec![Action::WorkerPanicked {
                message: "second".to_string(),
            }],
        });

        assert!(first_started.load(Ordering::SeqCst));
        assert!(second_started.load(Ordering::SeqCst));
        assert!(spawner.try_recv_completion().is_none());

        second_ready.store(true, Ordering::SeqCst);
        spawner.poll_tasks();
        first_ready.store(true, Ordering::SeqCst);
        spawner.poll_tasks();

        let Some(BackgroundCompletion::Succeeded(second_actions)) = spawner.try_recv_completion()
        else {
            panic!("expected second completion")
        };
        let Some(BackgroundCompletion::Succeeded(first_actions)) = spawner.try_recv_completion()
        else {
            panic!("expected first completion")
        };
        assert!(
            matches!(&second_actions[..], [Action::WorkerPanicked { message }] if message == "second")
        );
        assert!(
            matches!(&first_actions[..], [Action::WorkerPanicked { message }] if message == "first")
        );
        assert!(spawner.try_recv_completion().is_none());
    }
}
