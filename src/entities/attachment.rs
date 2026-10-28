use chrono::{DateTime, Local};

use crate::vos::AttachmentId;

/// Issueに添付されたファイルのメタデータ。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub id: AttachmentId,
    pub filename: String,
    pub filesize: u64,
    pub content_type: Option<String>,
    pub description: String,
    pub content_url: String,
    /// Redmineがサムネイルを作れる添付ファイルか。Issue詳細の`thumbnail_url`の有無で決まる。
    pub thumbnailable: bool,
    pub author: String,
    pub created_on: DateTime<Local>,
}
