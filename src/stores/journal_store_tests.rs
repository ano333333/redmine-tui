use super::Store;
use crate::vos::{JournalId, JournalKey, LocalJournalId};

#[test]
fn empty_store_has_no_remote_or_local_entry() {
    let store = Store::new();

    assert!(
        store
            .get_journal_entry(JournalKey::Remote(JournalId::new(1)))
            .is_none()
    );
    assert!(
        store
            .get_journal_entry(JournalKey::Local(LocalJournalId::new(1)))
            .is_none()
    );
}
