//! Remote JournalとLocal Journalを所有するIssue単位で保持するStore。

use std::collections::{HashMap, HashSet};

use super::journal_state::{
    JournalUploadFailure, LocalJournalEntry, LocalJournalState, RemoteJournalEntry,
    RemoteJournalState, RemoteJournalUploadConflict,
};
use crate::entities::{Journal, LocalJournal};
use crate::vos::{IssueId, JournalId, JournalNotesDiff};

/// Issueごとに、順序付きのRemote Journalと0件または1件のLocal Journalを管理する。
pub struct JournalStore {
    pub(super) by_issue: HashMap<IssueId, IssueJournals>,
}

pub(super) struct IssueJournals {
    pub(super) remote: Vec<RemoteJournalEntry>,
    pub(super) local: Option<LocalJournalEntry>,
}

impl IssueJournals {
    fn other_uploading_journal_exists(&self, excluded_journal_id: Option<JournalId>) -> bool {
        matches!(
            self.local.as_ref().map(|entry| &entry.state),
            Some(LocalJournalState::Uploading)
        ) || self.remote.iter().any(|entry| {
            !matches!(excluded_journal_id, Some(id) if entry.journal.id == id)
                && matches!(entry.state, RemoteJournalState::Uploading { .. })
        })
    }
}

/// JournalStoreの状態更新を表すAction。
pub enum JournalAction {
    /// 取得したRemote JournalをIssue単位で同期する。
    ///
    /// 編集中・upload中のRemote Journalは未保存の作業内容を失わないよう取得値で上書きせず、
    /// 取得結果から消えていても削除しない。
    SyncFetched {
        issue_id: IssueId,
        journals: Vec<Journal>,
    },
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
    /// 対象が未登録の場合、またはLocalOnly以外の状態の場合はpanicする。
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
    /// 対象が未登録の場合、またはEdited以外の状態の場合はpanicする。
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
    /// 作成されたJournalはRemote側にしか現れないため、`SyncFetched`と同じ同期規則で取り込みつつ、
    /// 同じAction内でLocal Journalを削除して二重表示を避ける。同期規則は`SyncFetched`と同じく、
    /// 編集中・upload中のRemote Journalを取得値で上書きしない。
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    /// FIXME: Entity定義と、1トランザクションとしてのActionの区切りが曖昧
    /// 整合性・結果整合性を考えてActionを分割するかどうか検討する
    CompleteLocalUploadWithFetched {
        issue_id: IssueId,
        journals: Vec<Journal>,
    },
}

impl JournalStore {
    /// Journalが未登録の空Storeを生成する。
    pub fn new() -> Self {
        Self {
            by_issue: HashMap::new(),
        }
    }

    pub(super) fn consume_action(&mut self, action: JournalAction) {
        match action {
            JournalAction::SyncFetched { issue_id, journals } => {
                Self::assert_sync_fetched_is_valid(self, issue_id, &journals);
                let issue_journals =
                    self.by_issue
                        .entry(issue_id)
                        .or_insert_with(|| IssueJournals {
                            remote: vec![],
                            local: None,
                        });
                let remote = std::mem::take(&mut issue_journals.remote);
                issue_journals.remote = Self::merge_sync_fetched(remote, journals);
            }
            JournalAction::CreateLocal { issue_id } => {
                let issue_journals =
                    self.by_issue
                        .entry(issue_id)
                        .or_insert_with(|| IssueJournals {
                            remote: vec![],
                            local: None,
                        });
                if issue_journals.local.is_some() {
                    panic!("local journal is already registered for issue {issue_id}");
                }
                issue_journals.local = Some(LocalJournalEntry {
                    journal: LocalJournal {
                        issue_id,
                        notes: String::new(),
                    },
                    state: LocalJournalState::LocalOnly { failure: None },
                });
            }
            JournalAction::EditLocalNotes { issue_id, notes } => {
                let entry = self
                    .by_issue
                    .get_mut(&issue_id)
                    .and_then(|issue_journals| issue_journals.local.as_mut())
                    .unwrap_or_else(|| {
                        panic!("local journal is not registered for issue {issue_id}")
                    });
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
                let issue_journals = self.by_issue.get_mut(&issue_id).unwrap_or_else(|| {
                    panic!("local journal is not registered for issue {issue_id}")
                });
                let uploading_others = issue_journals.other_uploading_journal_exists(None);
                let local = issue_journals.local.as_mut().unwrap_or_else(|| {
                    panic!("local journal is not registered for issue {issue_id}")
                });
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
                let entry = self
                    .by_issue
                    .get_mut(&issue_id)
                    .and_then(|issue_journals| issue_journals.local.as_mut())
                    .unwrap_or_else(|| {
                        panic!("local journal is not registered for issue {issue_id}")
                    });
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
                let entry = self.entry_mut(issue_id, journal_id);
                match &mut entry.state {
                    RemoteJournalState::Uploading { .. } => {
                        panic!("cannot edit remote journal {journal_id} while it is uploading");
                    }
                    RemoteJournalState::Synced => {
                        entry.state =
                            Self::edited_or_synced_state(entry.journal.notes.clone(), notes);
                    }
                    RemoteJournalState::Edited { diff, .. } => {
                        // 最初の取得値を比較基準に維持し、再編集前のupload失敗は新しい編集結果へ引き継がない。
                        entry.state = Self::edited_or_synced_state(diff.before.clone(), notes);
                    }
                }
            }
            JournalAction::StartRemoteUpload {
                issue_id,
                journal_id,
            } => {
                // 対象自身の不正な遷移は、他Journalとの排他関係によらない自己矛盾として
                // 先に検出する。他Journalとの排他は正常な遷移元であるEditedにのみ適用する。
                let issue_journals = self.by_issue.get_mut(&issue_id).unwrap_or_else(|| {
                    panic!("remote journal {journal_id} is not registered for issue {issue_id}")
                });
                let target_index = issue_journals
                    .remote
                    .iter()
                    .position(|entry| entry.journal.id == journal_id)
                    .unwrap_or_else(|| {
                        panic!("remote journal {journal_id} is not registered for issue {issue_id}")
                    });
                let diff = match &issue_journals.remote[target_index].state {
                    RemoteJournalState::Synced => {
                        panic!("cannot start remote journal upload while it is synced")
                    }
                    RemoteJournalState::Uploading { .. } => {
                        panic!("cannot start remote journal upload while it is uploading")
                    }
                    RemoteJournalState::Edited { diff, .. } => diff,
                };
                if issue_journals.other_uploading_journal_exists(Some(journal_id)) {
                    panic!(
                        "cannot start remote journal upload while another journal of issue {issue_id} is uploading"
                    );
                }
                issue_journals.remote[target_index].state = RemoteJournalState::Uploading {
                    diff: diff.clone(),
                    conflict: None,
                };
            }
            JournalAction::CompleteRemoteUpload {
                issue_id,
                journal_id,
                notes,
            } => {
                let entry = self.entry_mut(issue_id, journal_id);
                match &mut entry.state {
                    RemoteJournalState::Uploading { .. } => {
                        entry.journal.notes = notes;
                        entry.state = RemoteJournalState::Synced;
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
            }
            JournalAction::FailRemoteUpload {
                issue_id,
                journal_id,
                message,
            } => {
                let entry = self.entry_mut(issue_id, journal_id);
                match &mut entry.state {
                    RemoteJournalState::Uploading { diff, .. } => {
                        entry.state = RemoteJournalState::Edited {
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
            } => {
                let entry = self.entry_mut(issue_id, journal_id);
                match &mut entry.state {
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
                }
            }
            JournalAction::CancelRemoteUploadConflict {
                issue_id,
                journal_id,
            } => {
                let entry = self.entry_mut(issue_id, journal_id);
                match &mut entry.state {
                    RemoteJournalState::Uploading { diff, conflict } => {
                        if conflict.is_none() {
                            panic!(
                                "cannot cancel remote journal {journal_id} upload conflict while it is uploading without a conflict"
                            );
                        }
                        entry.state = RemoteJournalState::Edited {
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
                let entry = self.entry_mut(issue_id, journal_id);
                match &entry.state {
                    RemoteJournalState::Uploading { .. } => {}
                    RemoteJournalState::Synced => {
                        panic!("cannot remove remote journal {journal_id} while it is synced");
                    }
                    RemoteJournalState::Edited { .. } => {
                        panic!("cannot remove remote journal {journal_id} while it is edited");
                    }
                }
                let issue_journals = self.by_issue.get_mut(&issue_id).unwrap_or_else(|| {
                    panic!("remote journal {journal_id} is not registered for issue {issue_id}")
                });
                issue_journals
                    .remote
                    .retain(|entry| entry.journal.id != journal_id);
            }
            JournalAction::CompleteLocalUploadWithFetched { issue_id, journals } => {
                let issue_journals = self.by_issue.get_mut(&issue_id).unwrap_or_else(|| {
                    panic!("local journal is not registered for issue {issue_id}")
                });
                // 取得値のmergeとLocal Journalの削除は不可分に扱うため、片方だけが適用された状態を
                // 残さないよう、状態を変更する前にまとめて検証する。
                match issue_journals.local {
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
                let remote = std::mem::take(&mut issue_journals.remote);
                issue_journals.remote = Self::merge_sync_fetched(remote, journals);
                issue_journals.local = None;
            }
        }
    }

    fn entry_mut(&mut self, issue_id: IssueId, journal_id: JournalId) -> &mut RemoteJournalEntry {
        self.by_issue
            .get_mut(&issue_id)
            .and_then(|issue_journals| {
                issue_journals
                    .remote
                    .iter_mut()
                    .find(|entry| entry.journal.id == journal_id)
            })
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

    // Storeへ到達したSyncFetchedのIDと所有関係の不整合は、取得失敗ではなくAction生成側の制御破綻として拒否する。
    fn assert_sync_fetched_is_valid(this: &JournalStore, issue_id: IssueId, journals: &[Journal]) {
        let mut seen: HashSet<JournalId> = HashSet::with_capacity(journals.len());
        for journal in journals {
            let journal_id = journal.id;
            if !seen.insert(journal_id) {
                panic!(
                    "sync fetched journals for issue {issue_id} contain duplicate journal {journal_id}"
                );
            }
            if journal.issue_id != issue_id {
                panic!(
                    "sync fetched journal {journal_id} of issue {issue_id} has issue {}",
                    journal.issue_id
                );
            }
        }
        for journal in journals {
            let registered_other_issue_id = this
                .by_issue
                .iter()
                .find(|(other_issue_id, issue_journals)| {
                    **other_issue_id != issue_id
                        && issue_journals
                            .remote
                            .iter()
                            .any(|entry| entry.journal.id == journal.id)
                })
                .map(|(other_issue_id, _)| *other_issue_id);
            if let Some(other_issue_id) = registered_other_issue_id {
                panic!(
                    "remote journal {} is already registered for issue {other_issue_id}",
                    journal.id
                );
            }
        }
    }

    fn merge_sync_fetched(
        current: Vec<RemoteJournalEntry>,
        fetched: Vec<Journal>,
    ) -> Vec<RemoteJournalEntry> {
        // 表示順は取得順を優先し、取得結果にないdirty entryは以前の相対順で末尾に残す。
        let dirty_entry_order: Vec<JournalId> = current
            .iter()
            .filter(|entry| !matches!(entry.state, RemoteJournalState::Synced))
            .map(|entry| entry.journal.id)
            .collect();
        let mut by_id: HashMap<JournalId, RemoteJournalEntry> =
            HashMap::with_capacity(current.len());
        for entry in current {
            by_id.insert(entry.journal.id, entry);
        }
        let mut merged = Vec::with_capacity(fetched.len());
        for journal in fetched {
            if let Some(mut entry) = by_id.remove(&journal.id) {
                // Edited/Uploadingの未保存の作業内容は、取得値で上書きしない意図的なmerge no-opとする。
                if matches!(entry.state, RemoteJournalState::Synced) {
                    entry.journal = journal;
                }
                merged.push(entry);
            } else {
                merged.push(RemoteJournalEntry {
                    journal,
                    state: RemoteJournalState::Synced,
                });
            }
        }
        for journal_id in dirty_entry_order {
            if let Some(kept_dirty_entry) = by_id.remove(&journal_id) {
                merged.push(kept_dirty_entry);
            }
        }
        merged
    }

    /// IssueのRemote Journalを保持順に返す。
    ///
    /// Issueが未登録の場合は空のsliceを返す。
    pub fn get_remote_journals(&self, issue_id: impl Into<IssueId>) -> &[RemoteJournalEntry] {
        let issue_id = issue_id.into();
        self.by_issue
            .get(&issue_id)
            .map(|journals| journals.remote.as_slice())
            .unwrap_or(&[])
    }

    /// Issueに登録されているRemote Journalを返す。
    ///
    /// # Panics
    ///
    /// 指定したIssueに指定したRemote Journalが登録されていない場合にpanicする。
    pub fn get_remote_journal(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> &RemoteJournalEntry {
        let issue_id = issue_id.into();
        let journal_id = journal_id.into();
        self.get_remote_journals(issue_id)
            .iter()
            .find(|entry| entry.journal.id == journal_id)
            .unwrap_or_else(|| {
                panic!("remote journal {journal_id} is not registered for issue {issue_id}")
            })
    }

    /// 指定したRemote JournalがIssueに登録されているかを返す。
    pub fn has_remote_journal(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> bool {
        let journal_id = journal_id.into();
        self.get_remote_journals(issue_id)
            .iter()
            .any(|entry| entry.journal.id == journal_id)
    }

    /// 競合解決に必要な編集差分と、保存前確認で取得したサーバー値を返す。
    ///
    /// 対象が未登録の場合、または競合中でない場合は`None`を返す。
    pub fn get_remote_journal_upload_conflict(
        &self,
        issue_id: impl Into<IssueId>,
        journal_id: impl Into<JournalId>,
    ) -> Option<(&JournalNotesDiff, &RemoteJournalUploadConflict)> {
        let issue_id = issue_id.into();
        let journal_id = journal_id.into();
        self.get_remote_journals(issue_id)
            .iter()
            .find(|entry| entry.journal.id == journal_id)
            .and_then(|entry| match &entry.state {
                RemoteJournalState::Uploading {
                    diff,
                    conflict: Some(conflict),
                } => Some((diff, conflict)),
                _ => None,
            })
    }

    /// Issueが持つ唯一のLocal Journalを返す。
    ///
    /// Issueが未登録の場合、またはLocal Journalを持たない場合は`None`を返す。
    pub fn get_local_journal(&self, issue_id: impl Into<IssueId>) -> Option<&LocalJournalEntry> {
        self.by_issue
            .get(&issue_id.into())
            .and_then(|journals| journals.local.as_ref())
    }

    /// 対象IssueのRemote JournalまたはLocal Journalがupload中かを返す。
    ///
    /// Journalが未登録のIssue、およびJournalがすべて待機中のIssueでは`false`を返す。
    /// 同一Issue内の別Journalとのupload排他を検査するために使用する。
    pub fn has_uploading_journal(&self, issue_id: impl Into<IssueId>) -> bool {
        let issue_id = issue_id.into();
        self.by_issue
            .get(&issue_id)
            .map(|journals| journals.other_uploading_journal_exists(None))
            .unwrap_or(false)
    }
}
