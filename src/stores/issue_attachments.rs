//! Issueへ添付ファイルを追加する処理の進行状態。

use crate::entities::{IssueAggregate, IssueChild};
use crate::vos::IssueId;

/// 1つの段階が、Redmineへの1つのリクエストに対応する。
///
/// `None`は追加中でないことを表す。`FailUpload`はどの段階からも`None`へ戻す。
///
/// ```text
///    ┌──────┐      StartUpload           ┌───────────┐
///    │ None │ ─────────────────────────▶ │ Uploading │
///    └──────┘ ◀──────────┐               └───────────┘
///       ▲                │                     │ UploadSucceeded
///       │                │ FailUpload          ▼
///       │                ├────────────── ┌───────────┐
///       │                │               │ Attaching │
///       │                │               └───────────┘
///       │                │                     │ AttachSucceeded
///       │                │                     ▼
///       │                └────────────── ┌────────────┐
///       └─────────────────────────────── │ Confirming │
///               ConfirmSucceeded         └────────────┘
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentUploadStage {
    /// ファイル本体を送っている。
    Uploading,
    /// 送ったファイルをIssueへ紐付けている。
    Attaching,
    /// 紐付けた結果を確かめるため、Issueを取得し直している。
    Confirming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentUpload {
    pub filename: String,
    pub stage: AttachmentUploadStage,
}

pub enum AttachmentAction {
    /// Issueが取得済みでない場合、またはIssue属性・Journal・別の添付ファイルを保存中の場合はpanicする。
    StartUpload { issue_id: IssueId, filename: String },
    /// Uploading以外の場合はpanicする。
    UploadSucceeded { issue_id: IssueId },
    /// Attaching以外の場合はpanicする。
    AttachSucceeded { issue_id: IssueId },
    /// 確認の取得結果を取り込み、追加状態を削除する。
    ///
    /// Issue属性の差分とJournalの作業は引き継ぐ。Confirming以外の場合はpanicする。
    ConfirmSucceeded {
        issue: IssueAggregate,
        children: Vec<IssueChild>,
    },
    /// どの段階で失敗しても再試行しないため、追加状態を削除する。追加中でない場合はpanicする。
    FailUpload { issue_id: IssueId },
}
