//! Issueへ添付ファイルを追加する処理の進行状態と、添付ファイルの表示に使う画像。

use super::image_store::ImageKey;
use crate::entities::{IssueAggregate, IssueChild};
use crate::vos::{AttachmentId, IssueId};

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

/// 添付ファイルの表示に使う画像を決め、その読み込みを始める。
///
/// 対応する画像の状態は`ImageStore`が持つ。1つのActionで、添付ファイルと画像の対応と、画像の状態を同時に更新する。
pub enum AttachmentImageAction {
    /// 添付ファイルの表示に`key`の画像を使い、その読み込みを始める。
    ///
    /// 添付ファイルに対応する画像があり、その画像が`Failed`でない場合はpanicする。
    /// `key`の画像が`Failed`でない状態を持つ場合もpanicする。
    Start {
        attachment_id: AttachmentId,
        key: ImageKey,
    },
    /// サムネイルを読み込めなかったため、そのサムネイルを`Failed`にし、原画像の読み込みへ切り替える。
    ///
    /// 対応する画像がLoading中のサムネイルでない場合はpanicする。
    FallBackToOriginal {
        attachment_id: AttachmentId,
        reason: String,
    },
}
