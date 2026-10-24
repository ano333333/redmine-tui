use std::sync::Arc;

/// Usecaseが外部とやり取りするために使うclient。
pub struct UsecaseClients<C, F> {
    pub redmine: Arc<C>,
    pub local_file: Arc<F>,
}

// `derive(Clone)`は`C`と`F`にも`Clone`を要求するため、Arcの複製だけで実装する。
impl<C, F> Clone for UsecaseClients<C, F> {
    fn clone(&self) -> Self {
        Self {
            redmine: self.redmine.clone(),
            local_file: self.local_file.clone(),
        }
    }
}
