use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;

use super::{BackgroundCompletion, BackgroundSpawner};

pub(crate) struct WebBackgroundSpawner<T> {
    completions: Rc<RefCell<VecDeque<BackgroundCompletion<T>>>>,
}

impl<T> WebBackgroundSpawner<T> {
    pub(crate) fn new() -> Self {
        Self {
            completions: Rc::new(RefCell::new(VecDeque::new())),
        }
    }
}

impl<T: Send + 'static> BackgroundSpawner for WebBackgroundSpawner<T> {
    type Output = T;

    fn spawn<F>(&self, task: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        let completions = Rc::clone(&self.completions);
        // wasm32ではpanicをunwindできないため、Panickedは生成せずentryのpanic hookへ報告を任せる。
        wasm_bindgen_futures::spawn_local(async move {
            let output = task.await;
            completions
                .borrow_mut()
                .push_back(BackgroundCompletion::Succeeded(output));
        });
    }

    fn try_recv_completion(&self) -> Option<BackgroundCompletion<T>> {
        self.completions.borrow_mut().pop_front()
    }
}
