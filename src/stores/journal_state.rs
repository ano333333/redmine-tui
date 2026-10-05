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

/// Issueが保持するRemote Journal本体と、そのJournalに固有の編集・保存状態への参照。
#[derive(Clone, Copy)]
pub struct RemoteJournalView<'a> {
    pub journal: &'a Journal,
    pub state: &'a RemoteJournalState,
}

/// Redmine由来のIDをまだ確認できていないJournalの保存状態。
#[derive(Clone, Debug, PartialEq)]
pub enum LocalJournalState {
    /// 投稿していない、または投稿のPUTが失敗した状態。
    ///
    /// 通信切断などでPUTの応答を受け取れずに失敗した場合は、サーバーに保存済みのことがある。
    LocalOnly {
        failure: Option<JournalUploadFailure>,
    },
    Uploading,
}

/// Local Journal本体と、そのJournalに固有の保存状態。
#[derive(Clone, Debug)]
pub struct LocalJournalEntry {
    pub journal: LocalJournal,
    pub state: LocalJournalState,
}
