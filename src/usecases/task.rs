use std::future::Future;
use std::pin::Pin;

use crate::stores::Action;

/// runnerがbackgroundで実行したUsecaseの完了値。
pub struct UsecaseOutput {
    pub actions: Vec<Action>,
}

impl From<Vec<Action>> for UsecaseOutput {
    fn from(actions: Vec<Action>) -> Self {
        Self { actions }
    }
}

pub type UsecaseTask = Pin<Box<dyn Future<Output = UsecaseOutput> + Send + 'static>>;
