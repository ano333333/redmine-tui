use super::JournalId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalJournalId(u64);

impl LocalJournalId {
    /// Creates a session-local ID issued by the Store.
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JournalKey {
    Remote(JournalId),
    Local(LocalJournalId),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{JournalKey, LocalJournalId};
    use crate::vos::JournalId;

    #[test]
    fn remote_and_local_keys_are_distinct() {
        let remote = JournalKey::Remote(JournalId::new(1));
        let local = JournalKey::Local(LocalJournalId::new(1));

        assert_ne!(remote, local);
        assert_eq!(HashSet::from([remote, local]).len(), 2);
    }

    #[test]
    fn local_id_retains_store_issued_value() {
        let id = LocalJournalId::new(42);

        assert_eq!(id.0, 42);
    }
}
