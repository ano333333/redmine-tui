//! Remote JournalとLocal Journalを所有するIssue単位で保持するStore。

use std::collections::{HashMap, HashSet};

use super::journal_state::{LocalJournalEntry, RemoteJournalEntry, RemoteJournalState};
use crate::entities::Journal;
use crate::vos::{IssueId, JournalId, JournalNotesDiff};

/// Issueごとに、順序付きのRemote Journalと0件または1件のLocal Journalを管理する。
pub struct JournalStore {
    pub(super) by_issue: HashMap<IssueId, IssueJournals>,
}

pub(super) struct IssueJournals {
    pub(super) remote: Vec<RemoteJournalEntry>,
    pub(super) local: Option<LocalJournalEntry>,
}

/// JournalStoreの状態更新を表すAction。
pub enum JournalAction {
    /// 取得したRemote JournalをIssue単位で同期する。
    SyncFetched {
        issue_id: IssueId,
        journals: Vec<Journal>,
    },
    /// Remote Journalのnotes編集結果を状態へ反映する。
    ///
    /// 対象が未登録の場合、またはupload中の場合はpanicする。
    EditRemoteNotes {
        issue_id: IssueId,
        journal_id: JournalId,
        notes: String,
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
            JournalAction::EditRemoteNotes {
                issue_id,
                journal_id,
                notes,
            } => {
                let entry = self
                    .by_issue
                    .get_mut(&issue_id)
                    .and_then(|issue_journals| {
                        issue_journals
                            .remote
                            .iter_mut()
                            .find(|entry| entry.journal.id == journal_id)
                    })
                    .unwrap_or_else(|| {
                        panic!("remote journal {journal_id} is not registered for issue {issue_id}")
                    });
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
        }
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

    /// Issueが持つ唯一のLocal Journalを返す。
    ///
    /// Issueが未登録の場合、またはLocal Journalを持たない場合は`None`を返す。
    pub fn get_local_journal(&self, issue_id: impl Into<IssueId>) -> Option<&LocalJournalEntry> {
        self.by_issue
            .get(&issue_id.into())
            .and_then(|journals| journals.local.as_ref())
    }
}
