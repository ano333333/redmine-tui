//! Remote JournalとLocal Journalを所有するIssue単位で保持するStore。

use std::collections::HashMap;

use super::journal_state::{LocalJournalEntry, RemoteJournalEntry};
use crate::vos::{IssueId, JournalId};

/// Issueごとに、順序付きのRemote Journalと0件または1件のLocal Journalを管理する。
pub struct JournalStore {
    pub(super) by_issue: HashMap<IssueId, IssueJournals>,
}

pub(super) struct IssueJournals {
    pub(super) remote: Vec<RemoteJournalEntry>,
    pub(super) local: Option<LocalJournalEntry>,
}

impl JournalStore {
    /// Journalが未登録の空Storeを生成する。
    pub fn new() -> Self {
        Self {
            by_issue: HashMap::new(),
        }
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
