use std::future::Future;
use std::pin::Pin;

use crate::stores::Action;
use crate::usecases::{UsecaseEvent, UsecaseRequest};

pub struct UsecaseOutput {
    pub actions: Vec<Action>,
    pub requests: Vec<UsecaseRequest>,
    pub events: Vec<UsecaseEvent>,
}

impl From<Vec<Action>> for UsecaseOutput {
    fn from(actions: Vec<Action>) -> Self {
        Self {
            actions,
            requests: Vec::new(),
            events: Vec::new(),
        }
    }
}

pub type UsecaseTask = Pin<Box<dyn Future<Output = UsecaseOutput> + Send + 'static>>;
