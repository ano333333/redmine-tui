//! Issueレスポンスに埋め込まれた添付ファイルのDTOをドメイン型へ変換する。

use serde::Deserialize;

use crate::clients::redmine::RedmineClientError;
use crate::entities::Attachment;
use crate::vos::AttachmentId;

use super::value_conversion;

/// Redmineの添付ファイルレスポンスを受け取る中間DTO。
#[derive(Deserialize)]
pub(super) struct RedmineAttachment {
    pub(super) id: u16,
    pub(super) filename: String,
    pub(super) filesize: u64,
    #[serde(default)]
    pub(super) content_type: Option<String>,
    #[serde(default)]
    pub(super) description: Option<String>,
    pub(super) content_url: String,
    pub(super) author: RedmineAttachmentAuthor,
    pub(super) created_on: String,
}

#[derive(Deserialize)]
pub(super) struct RedmineAttachmentAuthor {
    pub(super) name: String,
}

/// Content-Typeは、空文字とnullのどちらも未設定として`None`にする。
/// `created_on`がRFC 3339形式でなければ[`RedmineClientError::Client`]を返す。
pub(super) fn convert_attachment(
    attachment: RedmineAttachment,
) -> Result<Attachment, RedmineClientError> {
    Ok(Attachment {
        id: AttachmentId::new(attachment.id),
        filename: attachment.filename,
        filesize: attachment.filesize,
        content_type: attachment
            .content_type
            .filter(|content_type| !content_type.is_empty()),
        description: attachment.description.unwrap_or_default(),
        content_url: attachment.content_url,
        author: attachment.author.name,
        created_on: value_conversion::parse_datetime(&attachment.created_on)?,
    })
}
