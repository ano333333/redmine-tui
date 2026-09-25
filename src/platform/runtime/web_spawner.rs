use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;

use super::{BackgroundCompletion, BackgroundSpawner};
use crate::stores::Action;

pub(crate) struct WebBackgroundSpawner {
    completions: Rc<RefCell<VecDeque<BackgroundCompletion>>>,
}

impl WebBackgroundSpawner {
    pub(crate) fn new() -> Self {
        Self {
            completions: Rc::new(RefCell::new(VecDeque::new())),
        }
    }
}

impl BackgroundSpawner for WebBackgroundSpawner {
    fn spawn<F>(&self, task: F)
    where
        F: Future<Output = Vec<Action>> + Send + 'static,
    {
        let completions = Rc::clone(&self.completions);
        // wasm32ではpanicをunwindできないため、Panickedは生成せずentryのpanic hookへ報告を任せる。
        wasm_bindgen_futures::spawn_local(async move {
            let actions = task.await;
            completions
                .borrow_mut()
                .push_back(BackgroundCompletion::Succeeded(actions));
        });
    }

    fn try_recv_completion(&self) -> Option<BackgroundCompletion> {
        self.completions.borrow_mut().pop_front()
    }
}
