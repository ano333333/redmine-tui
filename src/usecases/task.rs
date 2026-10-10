use std::future::Future;
use std::pin::Pin;

use crate::stores::Action;
use crate::usecases::UsecaseRequest;

/// runnerがbackgroundで実行したUsecaseの完了値。
pub struct UsecaseOutput {
    pub actions: Vec<Action>,
    pub requests: Vec<UsecaseRequest>,
}

impl From<Vec<Action>> for UsecaseOutput {
    fn from(actions: Vec<Action>) -> Self {
        Self {
            actions,
            requests: Vec::new(),
        }
    }
}

pub type UsecaseTask = Pin<Box<dyn Future<Output = UsecaseOutput> + Send + 'static>>;
