/// Remote Journal の編集前後とサーバー値を照合した保存方針。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteJournalUploadResolution {
    /// サーバーが編集前の値を保っているため、編集後の値を送信する。
    Upload,
    /// サーバーに編集後の値が反映済みのため、送信しない。
    AlreadyApplied,
    /// サーバーとローカルの双方が編集前の値から変化しているため、競合を解決する。
    Conflict,
}

/// Remote Journal の notes を完全一致で三者比較し、保存方針を返す。
pub fn resolve_remote_journal_upload(
    before: &str,
    after: &str,
    server: &str,
) -> RemoteJournalUploadResolution {
    // 編集前後が同じ場合も反映済みとみなし、不要な PUT を避ける。
    if server == after {
        RemoteJournalUploadResolution::AlreadyApplied
    } else if server == before {
        RemoteJournalUploadResolution::Upload
    } else {
        RemoteJournalUploadResolution::Conflict
    }
}

#[cfg(test)]
mod tests {
    use super::{RemoteJournalUploadResolution, resolve_remote_journal_upload};

    #[test]
    fn upload_when_server_equals_before() {
        assert_eq!(
            resolve_remote_journal_upload("before", "after", "before"),
            RemoteJournalUploadResolution::Upload
        );
    }

    #[test]
    fn already_applied_when_server_equals_after() {
        assert_eq!(
            resolve_remote_journal_upload("before", "after", "after"),
            RemoteJournalUploadResolution::AlreadyApplied
        );
    }

    #[test]
    fn conflict_when_server_equals_neither() {
        assert_eq!(
            resolve_remote_journal_upload("before", "after", "other"),
            RemoteJournalUploadResolution::Conflict
        );
    }

    #[test]
    fn upload_with_empty_strings() {
        assert_eq!(
            resolve_remote_journal_upload("", "", "x"),
            RemoteJournalUploadResolution::Conflict
        );
        assert_eq!(
            resolve_remote_journal_upload("", "x", ""),
            RemoteJournalUploadResolution::Upload
        );
        assert_eq!(
            resolve_remote_journal_upload("x", "", ""),
            RemoteJournalUploadResolution::AlreadyApplied
        );
    }

    #[test]
    fn all_empty_is_already_applied() {
        assert_eq!(
            resolve_remote_journal_upload("", "", ""),
            RemoteJournalUploadResolution::AlreadyApplied
        );
    }

    #[test]
    fn server_equal_to_after_takes_precedence_over_before() {
        assert_eq!(
            resolve_remote_journal_upload("same", "same", "same"),
            RemoteJournalUploadResolution::AlreadyApplied
        );
    }

    #[test]
    fn server_equal_to_before_takes_precedence_over_conflict() {
        assert_eq!(
            resolve_remote_journal_upload("before", "before", "before"),
            RemoteJournalUploadResolution::AlreadyApplied
        );
        assert_eq!(
            resolve_remote_journal_upload("before", "after", "before"),
            RemoteJournalUploadResolution::Upload
        );
    }

    #[test]
    fn conflict_with_empty_before_and_after() {
        assert_eq!(
            resolve_remote_journal_upload("", "", "server"),
            RemoteJournalUploadResolution::Conflict
        );
    }

    #[test]
    fn conflict_when_only_server_differs() {
        assert_eq!(
            resolve_remote_journal_upload("before", "after", "conflicting"),
            RemoteJournalUploadResolution::Conflict
        );
    }
}
