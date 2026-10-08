//! 取得済みIssueが持つJournalの編集・保存状態と、その状態を遷移させるAction。
//!
//! Journal本体は`IssueAggregate::journals`が所有し、ここでは本体ごとの状態、
//! 0件または1件のLocal Journal、取得結果から消えた編集中Journalの退避データを保持する。

use std::collections::{HashMap, HashSet};

use super::journal_state::{
    DeletedJournalEntry, DeletedJournalState, JournalUploadFailure, LocalJournalEntry,
    LocalJournalState, RemoteJournalState, RemoteJournalUploadConflict,
};
use crate::entities::{IssueAggregate, IssueChild, Journal, LocalJournal};
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
    /// 保存の完了後、取得したIssueを取り込み、対象をSyncedへ遷移する。
    ///
    /// 対象の本体も取得値にし、取得結果に対象がなければ一覧から外す。Issue本体・他のJournal・
    /// 子一覧も取得値にし、Issue属性と他のJournalの編集差分は残す。
    /// Issueが取得済みでSyncedかEditedでない場合、対象が未登録の場合、またはUploading以外の
    /// 状態の場合はpanicする。
    CompleteRemoteUpload {
        journal_id: JournalId,
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    /// upload失敗後も編集差分を維持し、再試行可能なEditedへ戻す。
    ///
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    FailRemoteUpload {
        issue_id: IssueId,
        journal_id: JournalId,
        message: String,
    },
    /// 保存前の取得で競合を検出した結果として、取得したIssueを取り込み、対象の編集差分を
    /// 維持したまま取得したnotesを競合情報として保持する。
    ///
    /// 対象の本体は取得値で上書きしない。すでに競合情報がある場合は、より新しく取得した
    /// サーバー値で置き換える。Issueが取得済みでSyncedかEditedでない場合、対象が未登録の場合、
    /// Uploading以外の状態の場合、または取得結果に対象がない場合はpanicする。
    DetectRemoteUploadConflict {
        journal_id: JournalId,
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    /// 競合情報を破棄し、編集差分を維持した再試行可能なEditedへ戻す。
    ///
    /// 対象が未登録の場合、Uploading以外の状態の場合、または競合情報がない場合はpanicする。
    CancelRemoteUploadConflict {
        issue_id: IssueId,
        journal_id: JournalId,
    },
    /// 保存前の取得で対象が消えていた結果として、取得したIssueを取り込み、`notes`を退避する。
    ///
    /// 消えたIDへは保存できないため、退避データからの手動の投稿を待つ。Issueが取得済みで
    /// SyncedかEditedでない場合、対象が未登録の場合、Uploading以外の状態の場合、または
    /// 取得結果に対象がある場合はpanicする。
    EvacuateMissingRemoteUpload {
        journal_id: JournalId,
        notes: String,
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    /// Local Journalのupload成功後、取得したIssueの取り込みとLocal Journalの削除を一度に行う。
    ///
    /// 作成されたJournalはRemote側にしか現れないため、取得結果を取り込みつつ、同じAction内で
    /// Local Journalを削除して二重表示を避ける。Issue本体・Journal・子一覧はRemote Journalの
    /// 保存完了と同じく、Issue属性とJournalの編集差分を残して取得値にする。
    /// Issueが取得済みでSyncedかEditedでない場合、対象が未登録の場合、またはUploading以外の
    /// 状態の場合はpanicする。
    CompleteLocalUploadWithFetched {
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    /// 退避したJournalのnotesを置き換え、以前のupload失敗情報を破棄する。
    ///
    /// 対象が未登録の場合、またはupload中の場合はpanicする。
    EditDeletedNotes {
        issue_id: IssueId,
        original_id: JournalId,
        notes: String,
    },
    /// 退避したJournalを新規Journalとして投稿し始める。
    ///
    /// 対象が未登録の場合、Pending以外の状態の場合、Issue属性または同じIssueの別Journalが
    /// upload中の場合はpanicする。
    StartDeletedUpload {
        issue_id: IssueId,
        original_id: JournalId,
    },
    /// 投稿のPUT失敗後もnotesを維持し、失敗情報を保持したPendingへ戻す。
    ///
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    FailDeletedUpload {
        issue_id: IssueId,
        original_id: JournalId,
        message: String,
    },
    /// 投稿の成功後、取得したIssueの取り込みと対象の退避データの削除を一度に行う。
    ///
    /// Issue本体・Journal・子一覧はRemote Journalの保存完了と同じく取得値にする。
    /// Issueが取得済みでSyncedかEditedでない場合、対象が未登録の場合、またはUploading以外の
    /// 状態の場合はpanicする。
    CompleteDeletedUploadWithFetched {
        original_id: JournalId,
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    /// 投稿のPUT成功後、確認の取得に失敗した場合に対象の退避データを削除する。
    ///
    /// 対象が未登録の場合、またはUploading以外の状態の場合はpanicする。
    CompleteDeletedUploadWithoutFetch {
        issue_id: IssueId,
        original_id: JournalId,
    },
    /// 退避したJournalを投稿せずに破棄する。
    ///
    /// 対象が未登録の場合、またはupload中の場合はpanicする。
    DiscardDeleted {
        issue_id: IssueId,
        original_id: JournalId,
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
            | JournalAction::FailRemoteUpload { issue_id, .. }
            | JournalAction::CancelRemoteUploadConflict { issue_id, .. }
            | JournalAction::CompleteLocalUploadWithoutFetch { issue_id }
            | JournalAction::EditDeletedNotes { issue_id, .. }
            | JournalAction::StartDeletedUpload { issue_id, .. }
            | JournalAction::FailDeletedUpload { issue_id, .. }
            | JournalAction::CompleteDeletedUploadWithoutFetch { issue_id, .. }
            | JournalAction::DiscardDeleted { issue_id, .. } => *issue_id,
            JournalAction::CompleteRemoteUpload { issue, .. }
            | JournalAction::DetectRemoteUploadConflict { issue, .. }
            | JournalAction::EvacuateMissingRemoteUpload { issue, .. }
            | JournalAction::CompleteLocalUploadWithFetched { issue, .. }
            | JournalAction::CompleteDeletedUploadWithFetched { issue, .. } => issue.issue.id,
        }
    }
}

/// 取得済みIssueが持つ全Remote Journalの状態、0件または1件のLocal Journal、退避データ。
///
/// `remote`は`IssueAggregate::journals`の各Journalについて、Syncedも含めて必ず1件ずつ持つ。
/// `deleted`は退避した順に並び、元のIDは`remote`とも互いとも重複しない。
#[derive(Debug)]
pub(super) struct IssueJournalStates {
    remote: HashMap<JournalId, RemoteJournalState>,
    local: Option<LocalJournalEntry>,
    deleted: Vec<DeletedJournalEntry>,
}

impl IssueJournalStates {
    pub(super) fn synced(journals: &[Journal]) -> Self {
        Self {
            remote: journals
                .iter()
                .map(|journal| (journal.id, RemoteJournalState::Synced))
                .collect(),
            local: None,
            deleted: Vec::new(),
        }
    }

    pub(super) fn remote_state(&self, journal_id: JournalId) -> Option<&RemoteJournalState> {
        self.remote.get(&journal_id)
    }

    pub(super) fn local(&self) -> Option<&LocalJournalEntry> {
        self.local.as_ref()
    }

    pub(super) fn deleted(&self) -> &[DeletedJournalEntry] {
        &self.deleted
    }

    /// Remote JournalまたはLocal Journalのいずれかがupload中かを返す。
    pub(super) fn has_uploading(&self) -> bool {
        self.has_uploading_except(None)
    }

    fn has_uploading_except(&self, excluded_journal_id: Option<JournalId>) -> bool {
        matches!(
            self.local.as_ref().map(|entry| &entry.state),
            Some(LocalJournalState::Uploading)
        ) || self
            .deleted
            .iter()
            .any(|entry| matches!(entry.state, DeletedJournalState::Uploading))
            || self.remote.iter().any(|(journal_id, state)| {
                Some(*journal_id) != excluded_journal_id
                    && matches!(state, RemoteJournalState::Uploading { .. })
            })
    }

    /// 取得したJournal一覧を現在の本体と状態へ取り込み、新しい本体の一覧を返す。
    ///
    /// 取得結果から消えた編集中のJournalは退避し、退避済みの元IDが再び現れたら
    /// サーバーのnotesから退避したnotesへの編集として復帰させる。
    pub(super) fn merge_fetched(
        &mut self,
        current: Vec<Journal>,
        fetched: Vec<Journal>,
    ) -> Vec<Journal> {
        let fetched_ids: HashSet<JournalId> = fetched.iter().map(|journal| journal.id).collect();
        // 取り込みはupload中のJournalがない時か、保存対象の状態を取得結果に合わせて遷移させた
        // 後に行うため、消えたupload中のJournalが届くのは制御の破綻である。状態を書き換える前に拒否する。
        if let Some(journal) = current.iter().find(|journal| {
            matches!(
                self.remote.get(&journal.id),
                Some(RemoteJournalState::Uploading { .. })
            ) && !fetched_ids.contains(&journal.id)
        }) {
            panic!("fetched journals lack uploading journal {}", journal.id);
        }
        let mut current_by_id: HashMap<JournalId, Journal> = HashMap::with_capacity(current.len());
        for journal in current {
            match self.remote.get(&journal.id) {
                Some(RemoteJournalState::Edited { diff, .. })
                    if !fetched_ids.contains(&journal.id) =>
                {
                    self.deleted.push(DeletedJournalEntry {
                        original_id: journal.id,
                        notes: diff.after.clone(),
                        state: DeletedJournalState::Pending { failure: None },
                    });
                    self.remote.remove(&journal.id);
                }
                _ => {
                    current_by_id.insert(journal.id, journal);
                }
            }
        }
        let mut merged = Vec::with_capacity(fetched.len());
        for journal in fetched {
            if let Some(index) = self
                .deleted
                .iter()
                .position(|entry| entry.original_id == journal.id)
            {
                let restored = self.deleted.remove(index);
                self.remote.insert(
                    journal.id,
                    Self::edited_or_synced_state(journal.notes.clone(), restored.notes),
                );
                merged.push(journal);
                continue;
            }
            match current_by_id.remove(&journal.id) {
                // upload中の作業内容は、取得値で上書きしない意図的なmerge no-opとする。
                Some(kept)
                    if matches!(
                        self.remote.get(&kept.id),
                        Some(RemoteJournalState::Uploading { .. })
                    ) =>
                {
                    merged.push(kept)
                }
                Some(_)
                    if matches!(
                        self.remote.get(&journal.id),
                        Some(RemoteJournalState::Edited { .. })
                    ) =>
                {
                    // 本体は新しい取得値にするが、保存時の競合判定の基準となるdiff.beforeは置き換えない。
                    let state = self.remote.get_mut(&journal.id).expect("checked above");
                    if matches!(state, RemoteJournalState::Edited { diff, .. } if diff.after == journal.notes)
                    {
                        *state = RemoteJournalState::Synced;
                    }
                    merged.push(journal);
                }
                _ => {
                    self.remote
                        .entry(journal.id)
                        .or_insert(RemoteJournalState::Synced);
                    merged.push(journal);
                }
            }
        }
        for removed_synced in current_by_id.keys() {
            self.remote.remove(removed_synced);
        }
        merged
    }

    /// upload中の対象をSyncedへ遷移する。続く取り込みで本体を取得値にする。
    pub(super) fn complete_remote_upload(&mut self, issue_id: IssueId, journal_id: JournalId) {
        let state = self.remote_mut(issue_id, journal_id);
        match state {
            RemoteJournalState::Uploading { .. } => *state = RemoteJournalState::Synced,
            RemoteJournalState::Synced => {
                panic!("cannot complete remote journal {journal_id} upload while it is synced");
            }
            RemoteJournalState::Edited { .. } => {
                panic!("cannot complete remote journal {journal_id} upload while it is edited");
            }
        }
    }

    pub(super) fn detect_remote_upload_conflict(
        &mut self,
        issue_id: IssueId,
        journal_id: JournalId,
        server_notes: String,
    ) {
        match self.remote_mut(issue_id, journal_id) {
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

    /// upload中の対象を`notes`への編集に戻す。続く取り込みで、消えた編集中のJournalとして退避する。
    pub(super) fn return_missing_remote_upload(
        &mut self,
        issue_id: IssueId,
        journal_id: JournalId,
        notes: String,
    ) {
        let state = self.remote_mut(issue_id, journal_id);
        match state {
            RemoteJournalState::Uploading { diff, .. } => {
                *state = RemoteJournalState::Edited {
                    diff: JournalNotesDiff {
                        before: diff.before.clone(),
                        after: notes,
                    },
                    failure: None,
                };
            }
            RemoteJournalState::Synced => {
                panic!("cannot evacuate remote journal {journal_id} upload while it is synced");
            }
            RemoteJournalState::Edited { .. } => {
                panic!("cannot evacuate remote journal {journal_id} upload while it is edited");
            }
        }
    }

    /// upload中のLocal Journalを削除する。続く取り込みで、作成されたJournalが一覧に現れる。
    pub(super) fn complete_local_upload(&mut self, issue_id: IssueId) {
        match self.local_mut(issue_id).state {
            LocalJournalState::Uploading => {}
            // Uploadingでないなら未uploadのLocalOnlyであり、対応するRemote Journalが
            // 存在しないため、取得値にはLocal Journalの内容が含まれていない。
            LocalJournalState::LocalOnly { .. } => panic!(
                "cannot complete local journal upload for issue {issue_id} while it is local only"
            ),
        }
        self.local = None;
    }

    /// 投稿中の退避データを削除する。取得結果に元IDが現れても、投稿済みの退避データを
    /// 復帰させないよう、取り込みより先に削除する。
    pub(super) fn complete_deleted_upload(&mut self, issue_id: IssueId, original_id: JournalId) {
        self.uploading_deleted_mut(issue_id, original_id);
        self.deleted
            .retain(|entry| entry.original_id != original_id);
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
            JournalAction::CompleteRemoteUpload { .. }
            | JournalAction::DetectRemoteUploadConflict { .. }
            | JournalAction::EvacuateMissingRemoteUpload { .. }
            | JournalAction::CompleteLocalUploadWithFetched { .. }
            | JournalAction::CompleteDeletedUploadWithFetched { .. } => {
                unreachable!("IssueStore applies journal upload results with the fetched issue")
            }
            JournalAction::EditDeletedNotes {
                issue_id,
                original_id,
                notes,
            } => {
                let entry = self.deleted_mut(issue_id, original_id);
                match entry.state {
                    DeletedJournalState::Pending { .. } => {
                        entry.notes = notes;
                        entry.state = DeletedJournalState::Pending { failure: None };
                    }
                    DeletedJournalState::Uploading => panic!(
                        "cannot edit deleted journal {original_id} of issue {issue_id} while it is uploading"
                    ),
                }
            }
            JournalAction::StartDeletedUpload {
                issue_id,
                original_id,
            } => {
                if matches!(
                    self.deleted_mut(issue_id, original_id).state,
                    DeletedJournalState::Uploading
                ) {
                    panic!(
                        "cannot start deleted journal {original_id} upload of issue {issue_id} while it is uploading"
                    );
                }
                if self.has_uploading() {
                    panic!(
                        "cannot start deleted journal upload while another journal of issue {issue_id} is uploading"
                    );
                }
                self.deleted_mut(issue_id, original_id).state = DeletedJournalState::Uploading;
            }
            JournalAction::FailDeletedUpload {
                issue_id,
                original_id,
                message,
            } => {
                let entry = self.uploading_deleted_mut(issue_id, original_id);
                entry.state = DeletedJournalState::Pending {
                    failure: Some(JournalUploadFailure { message }),
                };
            }
            JournalAction::CompleteDeletedUploadWithoutFetch {
                issue_id,
                original_id,
            } => {
                self.uploading_deleted_mut(issue_id, original_id);
                self.deleted
                    .retain(|entry| entry.original_id != original_id);
            }
            JournalAction::DiscardDeleted {
                issue_id,
                original_id,
            } => {
                if matches!(
                    self.deleted_mut(issue_id, original_id).state,
                    DeletedJournalState::Uploading
                ) {
                    panic!(
                        "cannot discard deleted journal {original_id} of issue {issue_id} while it is uploading"
                    );
                }
                self.deleted
                    .retain(|entry| entry.original_id != original_id);
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

    fn deleted_mut(
        &mut self,
        issue_id: IssueId,
        original_id: JournalId,
    ) -> &mut DeletedJournalEntry {
        self.deleted
            .iter_mut()
            .find(|entry| entry.original_id == original_id)
            .unwrap_or_else(|| {
                panic!("deleted journal {original_id} is not registered for issue {issue_id}")
            })
    }

    fn uploading_deleted_mut(
        &mut self,
        issue_id: IssueId,
        original_id: JournalId,
    ) -> &mut DeletedJournalEntry {
        let entry = self.deleted_mut(issue_id, original_id);
        if !matches!(entry.state, DeletedJournalState::Uploading) {
            panic!(
                "cannot finish deleted journal {original_id} upload of issue {issue_id} while it is pending"
            );
        }
        entry
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
