//! 取得済みIssueが持つJournalの編集・保存状態と、その状態を遷移させるAction。
//!
//! Journal本体は`IssueAggregate::journals`が所有し、ここでは本体ごとの状態と
//! 0件または1件のLocal Journalを保持する。

use std::collections::HashMap;

use super::journal_state::{
    JournalUploadFailure, LocalJournalEntry, LocalJournalState, RemoteJournalState,
    RemoteJournalUploadConflict,
};
use crate::entities::{Journal, LocalJournal};
use crate::vos::{IssueId, JournalId, JournalNotesDiff};

/// 取得済みIssueのJournalの状態更新を表すAction。
///
/// 対象Issueが取得済みでない場合はpanicする。
pub enum JournalAction {
    /// 空のLocal Journalを未保存状態で作成する。
    ///
    /// 対象IssueにLocal Journalがすでに登録されている場合はpanicする。
    CreateLocal { issue_id: IssueId },
    /// Local Journalのnotesを置き換え、以前のupload失敗情報を破棄する。
    ///
    /// 対象が未登録の場合、またはupload中の場合はpanicする。
    EditLocalNotes { issue_id: IssueId, notes: String },
    /// Local Journalのuploadを開始し、以前の失敗情報を破棄する。
    ///
    /// 対象が未登録の場合、LocalOnly以外の状態の場合、Issue属性または同じIssueの別Journalが
    /// upload中の場合はpanicする。
    StartLocalUpload { issue_id: IssueId },
    /// upload失敗後もnotesを維持し、失敗情報を保持したLocalOnlyへ戻す。
    ///
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    FailLocalUpload { issue_id: IssueId, message: String },
    /// Remote Journalのnotes編集結果を状態へ反映する。
    ///
    /// 対象が未登録の場合、またはupload中の場合はpanicする。
    EditRemoteNotes {
        issue_id: IssueId,
        journal_id: JournalId,
        notes: String,
    },
    /// Remote Journalのuploadを開始し、以前の失敗情報を破棄する。
    ///
    /// 対象が未登録の場合、Edited以外の状態の場合、Issue属性または同じIssueの別Journalが
    /// upload中の場合はpanicする。
    StartRemoteUpload {
        issue_id: IssueId,
        journal_id: JournalId,
    },
    /// upload完了時のnotesを同期基準にしてSyncedへ遷移する。
    ///
    /// PUTレスポンスでは更新日時を取得できないため、`updated_on`は既存値を維持する。
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    CompleteRemoteUpload {
        issue_id: IssueId,
        journal_id: JournalId,
        notes: String,
    },
    /// upload失敗後も編集差分を維持し、再試行可能なEditedへ戻す。
    ///
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    FailRemoteUpload {
        issue_id: IssueId,
        journal_id: JournalId,
        message: String,
    },
    /// upload中の編集差分を維持したまま、三者比較で取得したサーバー値を競合情報として保持する。
    ///
    /// すでに競合情報がある場合は、より新しく取得したサーバー値で置き換える。
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    DetectRemoteUploadConflict {
        issue_id: IssueId,
        journal_id: JournalId,
        server_notes: String,
    },
    /// 競合情報を破棄し、編集差分を維持した再試行可能なEditedへ戻す。
    ///
    /// 対象が未登録の場合、Uploading以外の状態の場合、または競合情報がない場合はpanicする。
    CancelRemoteUploadConflict {
        issue_id: IssueId,
        journal_id: JournalId,
    },
    /// Remote Journalのupload中、保存前の再取得で消失が確定した対象を正常な同期結果として削除する。
    ///
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    RemoveMissingRemoteJournal {
        issue_id: IssueId,
        journal_id: JournalId,
    },
    /// Local Journalのupload成功後、取得したJournal一覧の同期とLocal Journalの削除を一度に行う。
    ///
    /// 作成されたJournalはRemote側にしか現れないため、取得結果を取り込みつつ、同じAction内で
    /// Local Journalを削除して二重表示を避ける。編集中・upload中のRemote Journalは取得値で
    /// 上書きせず、取得結果から消えていても削除しない。
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    CompleteLocalUploadWithFetched {
        issue_id: IssueId,
        journals: Vec<Journal>,
    },
    /// Local JournalのPUT成功後、確認の取得に失敗した場合に下書きを削除する。
    ///
    /// 再試行で同じnotesを二重に投稿しないよう、下書きは復元しない。投稿したJournalは、
    /// 次にIssueのJournalを取り込むまで一覧に現れない。
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    CompleteLocalUploadWithoutFetch { issue_id: IssueId },
}

impl JournalAction {
    pub(super) fn issue_id(&self) -> IssueId {
        match self {
            JournalAction::CreateLocal { issue_id }
            | JournalAction::EditLocalNotes { issue_id, .. }
            | JournalAction::StartLocalUpload { issue_id }
            | JournalAction::FailLocalUpload { issue_id, .. }
            | JournalAction::EditRemoteNotes { issue_id, .. }
            | JournalAction::StartRemoteUpload { issue_id, .. }
            | JournalAction::CompleteRemoteUpload { issue_id, .. }
            | JournalAction::FailRemoteUpload { issue_id, .. }
            | JournalAction::DetectRemoteUploadConflict { issue_id, .. }
            | JournalAction::CancelRemoteUploadConflict { issue_id, .. }
            | JournalAction::RemoveMissingRemoteJournal { issue_id, .. }
            | JournalAction::CompleteLocalUploadWithFetched { issue_id, .. }
            | JournalAction::CompleteLocalUploadWithoutFetch { issue_id } => *issue_id,
        }
    }
}

/// 取得済みIssueが持つ全Remote Journalの状態と、0件または1件のLocal Journal。
///
/// `remote`は`IssueAggregate::journals`の各Journalについて、Syncedも含めて必ず1件ずつ持つ。
#[derive(Debug)]
pub(super) struct IssueJournalStates {
    remote: HashMap<JournalId, RemoteJournalState>,
    local: Option<LocalJournalEntry>,
}

impl IssueJournalStates {
    pub(super) fn synced(journals: &[Journal]) -> Self {
        Self {
            remote: journals
                .iter()
                .map(|journal| (journal.id, RemoteJournalState::Synced))
                .collect(),
            local: None,
        }
    }

    pub(super) fn remote_state(&self, journal_id: JournalId) -> Option<&RemoteJournalState> {
        self.remote.get(&journal_id)
    }

    pub(super) fn local(&self) -> Option<&LocalJournalEntry> {
        self.local.as_ref()
    }

    /// Remote JournalまたはLocal Journalのいずれかがupload中かを返す。
    pub(super) fn has_uploading(&self) -> bool {
        self.has_uploading_except(None)
    }

    fn has_uploading_except(&self, excluded_journal_id: Option<JournalId>) -> bool {
        matches!(
            self.local.as_ref().map(|entry| &entry.state),
            Some(LocalJournalState::Uploading)
        ) || self.remote.iter().any(|(journal_id, state)| {
            Some(*journal_id) != excluded_journal_id
                && matches!(state, RemoteJournalState::Uploading { .. })
        })
    }

    /// 取得したJournal一覧を現在の本体と状態へ取り込み、新しい本体の一覧を返す。
    pub(super) fn merge_fetched(
        &mut self,
        current: Vec<Journal>,
        fetched: Vec<Journal>,
    ) -> Vec<Journal> {
        // 表示順は取得順を優先し、取得結果にないdirty entryは以前の相対順で末尾に残す。
        let dirty_order: Vec<JournalId> = current
            .iter()
            .map(|journal| journal.id)
            .filter(|journal_id| !self.is_synced(*journal_id))
            .collect();
        let mut current_by_id: HashMap<JournalId, Journal> = current
            .into_iter()
            .map(|journal| (journal.id, journal))
            .collect();
        let mut merged = Vec::with_capacity(fetched.len());
        for journal in fetched {
            match current_by_id.remove(&journal.id) {
                // Edited/Uploadingの未保存の作業内容は、取得値で上書きしない意図的なmerge no-opとする。
                Some(kept) if !self.is_synced(kept.id) => merged.push(kept),
                _ => {
                    self.remote
                        .entry(journal.id)
                        .or_insert(RemoteJournalState::Synced);
                    merged.push(journal);
                }
            }
        }
        for journal_id in dirty_order {
            if let Some(kept_dirty) = current_by_id.remove(&journal_id) {
                merged.push(kept_dirty);
            }
        }
        for removed_synced in current_by_id.keys() {
            self.remote.remove(removed_synced);
        }
        merged
    }

    fn is_synced(&self, journal_id: JournalId) -> bool {
        matches!(
            self.remote.get(&journal_id),
            Some(RemoteJournalState::Synced)
        )
    }

    /// JournalActionを適用する。`journals`は対象Issueが所有するJournal本体の一覧である。
    ///
    /// Issue属性のupload中かどうかと、Journalの所有関係は呼び出し側で検査済みとする。
    pub(super) fn consume_action(&mut self, journals: &mut Vec<Journal>, action: JournalAction) {
        match action {
            JournalAction::CreateLocal { issue_id } => {
                if self.local.is_some() {
                    panic!("local journal is already registered for issue {issue_id}");
                }
                self.local = Some(LocalJournalEntry {
                    journal: LocalJournal {
                        issue_id,
                        notes: String::new(),
                    },
                    state: LocalJournalState::LocalOnly { failure: None },
                });
            }
            JournalAction::EditLocalNotes { issue_id, notes } => {
                let entry = self.local_mut(issue_id);
                match &mut entry.state {
                    LocalJournalState::LocalOnly { failure } => {
                        entry.journal.notes = notes;
                        *failure = None;
                    }
                    LocalJournalState::Uploading => {
                        panic!(
                            "cannot edit local journal for issue {issue_id} while it is uploading"
                        );
                    }
                }
            }
            JournalAction::StartLocalUpload { issue_id } => {
                let uploading_others = self.has_uploading_except(None);
                let local = self.local_mut(issue_id);
                if matches!(local.state, LocalJournalState::Uploading) {
                    panic!(
                        "cannot start local journal upload for issue {issue_id} while it is uploading"
                    );
                }
                if uploading_others {
                    panic!(
                        "cannot start local journal upload while another journal of issue {issue_id} is uploading"
                    );
                }
                local.state = LocalJournalState::Uploading;
            }
            JournalAction::FailLocalUpload { issue_id, message } => {
                let entry = self.local_mut(issue_id);
                match entry.state {
                    LocalJournalState::Uploading => {
                        entry.state = LocalJournalState::LocalOnly {
                            failure: Some(JournalUploadFailure { message }),
                        };
                    }
                    LocalJournalState::LocalOnly { .. } => {
                        panic!(
                            "cannot fail local journal upload for issue {issue_id} while it is local only"
                        );
                    }
                }
            }
            JournalAction::EditRemoteNotes {
                issue_id,
                journal_id,
                notes,
            } => {
                let fetched_notes = Self::journal(journals, issue_id, journal_id).notes.clone();
                let state = self.remote_mut(issue_id, journal_id);
                match state {
                    RemoteJournalState::Uploading { .. } => {
                        panic!("cannot edit remote journal {journal_id} while it is uploading");
                    }
                    RemoteJournalState::Synced => {
                        *state = Self::edited_or_synced_state(fetched_notes, notes);
                    }
                    RemoteJournalState::Edited { diff, .. } => {
                        // 最初の取得値を比較基準に維持し、再編集前のupload失敗は新しい編集結果へ引き継がない。
                        *state = Self::edited_or_synced_state(diff.before.clone(), notes);
                    }
                }
            }
            JournalAction::StartRemoteUpload {
                issue_id,
                journal_id,
            } => {
                // 対象自身の不正な遷移は、他Journalとの排他関係によらない自己矛盾として
                // 先に検出する。他Journalとの排他は正常な遷移元であるEditedにのみ適用する。
                let diff = match self.remote_mut(issue_id, journal_id) {
                    RemoteJournalState::Synced => {
                        panic!("cannot start remote journal upload while it is synced")
                    }
                    RemoteJournalState::Uploading { .. } => {
                        panic!("cannot start remote journal upload while it is uploading")
                    }
                    RemoteJournalState::Edited { diff, .. } => diff.clone(),
                };
                if self.has_uploading_except(Some(journal_id)) {
                    panic!(
                        "cannot start remote journal upload while another journal of issue {issue_id} is uploading"
                    );
                }
                *self.remote_mut(issue_id, journal_id) = RemoteJournalState::Uploading {
                    diff,
                    conflict: None,
                };
            }
            JournalAction::CompleteRemoteUpload {
                issue_id,
                journal_id,
                notes,
            } => {
                let state = self.remote_mut(issue_id, journal_id);
                match state {
                    RemoteJournalState::Uploading { .. } => {
                        *state = RemoteJournalState::Synced;
                    }
                    RemoteJournalState::Synced => {
                        panic!(
                            "cannot complete remote journal {journal_id} upload while it is synced"
                        );
                    }
                    RemoteJournalState::Edited { .. } => {
                        panic!(
                            "cannot complete remote journal {journal_id} upload while it is edited"
                        );
                    }
                }
                Self::journal_mut(journals, issue_id, journal_id).notes = notes;
            }
            JournalAction::FailRemoteUpload {
                issue_id,
                journal_id,
                message,
            } => {
                let state = self.remote_mut(issue_id, journal_id);
                match state {
                    RemoteJournalState::Uploading { diff, .. } => {
                        *state = RemoteJournalState::Edited {
                            diff: diff.clone(),
                            failure: Some(JournalUploadFailure { message }),
                        };
                    }
                    RemoteJournalState::Synced => {
                        panic!("cannot fail remote journal {journal_id} upload while it is synced");
                    }
                    RemoteJournalState::Edited { .. } => {
                        panic!("cannot fail remote journal {journal_id} upload while it is edited");
                    }
                }
            }
            JournalAction::DetectRemoteUploadConflict {
                issue_id,
                journal_id,
                server_notes,
            } => match self.remote_mut(issue_id, journal_id) {
                RemoteJournalState::Uploading { conflict, .. } => {
                    *conflict = Some(RemoteJournalUploadConflict { server_notes });
                }
                RemoteJournalState::Synced => {
                    panic!(
                        "cannot detect remote journal {journal_id} upload conflict while it is synced"
                    );
                }
                RemoteJournalState::Edited { .. } => {
                    panic!(
                        "cannot detect remote journal {journal_id} upload conflict while it is edited"
                    );
                }
            },
            JournalAction::CancelRemoteUploadConflict {
                issue_id,
                journal_id,
            } => {
                let state = self.remote_mut(issue_id, journal_id);
                match state {
                    RemoteJournalState::Uploading { diff, conflict } => {
                        if conflict.is_none() {
                            panic!(
                                "cannot cancel remote journal {journal_id} upload conflict while it is uploading without a conflict"
                            );
                        }
                        *state = RemoteJournalState::Edited {
                            diff: diff.clone(),
                            failure: None,
                        };
                    }
                    RemoteJournalState::Synced => {
                        panic!(
                            "cannot cancel remote journal {journal_id} upload conflict while it is synced"
                        );
                    }
                    RemoteJournalState::Edited { .. } => {
                        panic!(
                            "cannot cancel remote journal {journal_id} upload conflict while it is edited"
                        );
                    }
                }
            }
            JournalAction::RemoveMissingRemoteJournal {
                issue_id,
                journal_id,
            } => {
                match self.remote_mut(issue_id, journal_id) {
                    RemoteJournalState::Uploading { .. } => {}
                    RemoteJournalState::Synced => {
                        panic!("cannot remove remote journal {journal_id} while it is synced");
                    }
                    RemoteJournalState::Edited { .. } => {
                        panic!("cannot remove remote journal {journal_id} while it is edited");
                    }
                }
                self.remote.remove(&journal_id);
                journals.retain(|journal| journal.id != journal_id);
            }
            JournalAction::CompleteLocalUploadWithFetched {
                issue_id,
                journals: fetched,
            } => {
                // 取得値のmergeとLocal Journalの削除は不可分に扱うため、片方だけが適用された状態を
                // 残さないよう、状態を変更する前に検証する。
                match &self.local {
                    Some(LocalJournalEntry {
                        state: LocalJournalState::Uploading,
                        ..
                    }) => {}
                    // Uploadingでないなら未uploadのLocalOnlyであり、対応するRemote Journalが
                    // 存在しないため、取得値にはLocal Journalの内容が含まれていない。
                    Some(_) => {
                        panic!(
                            "cannot complete local journal upload for issue {issue_id} while it is local only"
                        );
                    }
                    None => {
                        panic!("local journal is not registered for issue {issue_id}");
                    }
                }
                let current = std::mem::take(journals);
                *journals = self.merge_fetched(current, fetched);
                self.local = None;
            }
            JournalAction::CompleteLocalUploadWithoutFetch { issue_id } => {
                match self.local_mut(issue_id).state {
                    LocalJournalState::Uploading => {}
                    LocalJournalState::LocalOnly { .. } => panic!(
                        "cannot complete local journal upload for issue {issue_id} while it is local only"
                    ),
                }
                self.local = None;
            }
        }
    }

    fn local_mut(&mut self, issue_id: IssueId) -> &mut LocalJournalEntry {
        self.local
            .as_mut()
            .unwrap_or_else(|| panic!("local journal is not registered for issue {issue_id}"))
    }

    fn remote_mut(&mut self, issue_id: IssueId, journal_id: JournalId) -> &mut RemoteJournalState {
        self.remote.get_mut(&journal_id).unwrap_or_else(|| {
            panic!("remote journal {journal_id} is not registered for issue {issue_id}")
        })
    }

    fn journal(journals: &[Journal], issue_id: IssueId, journal_id: JournalId) -> &Journal {
        journals
            .iter()
            .find(|journal| journal.id == journal_id)
            .unwrap_or_else(|| {
                panic!("remote journal {journal_id} is not registered for issue {issue_id}")
            })
    }

    fn journal_mut(
        journals: &mut [Journal],
        issue_id: IssueId,
        journal_id: JournalId,
    ) -> &mut Journal {
        journals
            .iter_mut()
            .find(|journal| journal.id == journal_id)
            .unwrap_or_else(|| {
                panic!("remote journal {journal_id} is not registered for issue {issue_id}")
            })
    }

    fn edited_or_synced_state(before: String, after: String) -> RemoteJournalState {
        if before == after {
            RemoteJournalState::Synced
        } else {
            RemoteJournalState::Edited {
                diff: JournalNotesDiff { before, after },
                failure: None,
            }
        }
    }
}
