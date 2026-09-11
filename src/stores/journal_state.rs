//! Remote JournalとLocal Journalそれぞれのentityと保存状態を結び付ける型。

use crate::entities::{Journal, LocalJournal};
use crate::vos::JournalNotesDiff;

/// Journalの保存処理で発生した、ユーザーへ通知する復帰可能な失敗。
///
/// 失敗した処理段階は状態分岐には使わず、必要な違いは`message`で表す。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalUploadFailure {
    pub message: String,
}

/// Remote Journalの保存前確認で検出した競合。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteJournalUploadConflict {
    pub server_notes: String,
}

/// Redmineに存在するJournalの編集・保存状態。
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteJournalState {
    Synced,
    Edited {
        diff: JournalNotesDiff,
        failure: Option<JournalUploadFailure>,
    },
    Uploading {
        diff: JournalNotesDiff,
        conflict: Option<RemoteJournalUploadConflict>,
    },
}

/// Remote Journal本体と、そのJournalに固有の編集・保存状態。
#[derive(Clone)]
pub struct RemoteJournalEntry {
    pub journal: Journal,
    pub state: RemoteJournalState,
}

/// Redmine由来のIDをまだ確認できていないJournalの保存状態。
#[derive(Clone, Debug, PartialEq)]
pub enum LocalJournalState {
    /// 対応するRemote JournalをStoreでまだ確認できていない状態。
    ///
    /// PUT後の再取得だけが失敗した場合もこの状態へ戻るため、未送信とは限らない。
    LocalOnly {
        failure: Option<JournalUploadFailure>,
    },
    Uploading,
}

/// Local Journal本体と、そのJournalに固有の保存状態。
#[derive(Clone)]
pub struct LocalJournalEntry {
    pub journal: LocalJournal,
    pub state: LocalJournalState,
}
