//! Remote JournalとLocal Journalを所有するIssue単位で保持するStore。

use std::collections::HashMap;

use super::journal_state::{LocalJournalEntry, RemoteJournalEntry, RemoteJournalState};
use crate::entities::Journal;
use crate::vos::{IssueId, JournalId};

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
        }
    }

    // 取得順を表示順として採用し、取得結果にないentryは現在の状態にかかわらず除外する。
    // TODO: dirty mergeでは、取得結果にないEdited/Uploading entryを以前の相対順で残す。
    fn merge_sync_fetched(
        current: Vec<RemoteJournalEntry>,
        fetched: Vec<Journal>,
    ) -> Vec<RemoteJournalEntry> {
        let mut by_id: HashMap<JournalId, RemoteJournalEntry> = HashMap::new();
        for entry in current {
            by_id.insert(entry.journal.id, entry);
        }
        let mut merged = Vec::with_capacity(fetched.len());
        for journal in fetched {
            if let Some(mut entry) = by_id.remove(&journal.id) {
                // Edited/Uploadingが保持するローカルの作業内容を取得値で上書きしない。
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
        merged
    }

    /// Journal IDだけでRemote Journalを検索する一時的な互換getter。
    ///
    /// Issue詳細UIがIssue IDによる検索へ移行するPhase 2 Step 2.7で削除する。
    pub(super) fn get_journal_by_id(&self, journal_id: impl Into<JournalId>) -> Option<&Journal> {
        let journal_id = journal_id.into();
        self.by_issue.values().find_map(|issue_journals| {
            issue_journals
                .remote
                .iter()
                .find(|entry| entry.journal.id == journal_id)
                .map(|entry| &entry.journal)
        })
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
