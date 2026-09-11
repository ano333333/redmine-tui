//! RedmineのJournal detail JSONのうち、IDまたは文字列で表されるIssue属性を変換する。

use serde::Deserialize;

use crate::clients::redmine::RedmineClientError;
use crate::vos::{IssueStatusId, JournalDetailAttr, PriorityId, ProjectId, TrackerId, UserId};

/// RedmineのJournal detailレスポンスを受け取る中間DTO。
///
/// `old_value`と`new_value`はRedmineが省略することがあるため、ドメイン型への
/// 変換時に空文字列へ正規化する。
#[derive(Deserialize)]
pub(super) struct RedmineJournalDetail {
    pub(super) property: String,
    pub(super) name: String,
    pub(super) old_value: Option<String>,
    pub(super) new_value: Option<String>,
}

/// Journal detail DTOを、対応するIssue属性の変更内容へ変換する。
///
/// `property`が`attr`でないdetailは、このクライアントの変換対象外として正常に
/// 読み飛ばし、`Ok(None)`を返す。`attr`のうち対応する属性は`Ok(Some(_))`へ変換し、
/// 未対応の属性名、またはID属性に数値以外の値が含まれる場合は
/// [`RedmineClientError::Client`]を返す。
pub(super) fn try_into_domain(
    detail: RedmineJournalDetail,
) -> Result<Option<JournalDetailAttr>, RedmineClientError> {
    if detail.property != "attr" {
        return Ok(None);
    }

    let old_value = detail.old_value.unwrap_or_default();
    let new_value = detail.new_value.unwrap_or_default();

    let attr = match detail.name.as_str() {
        "status_id" => JournalDetailAttr::StatusId {
            old: IssueStatusId::new(parse_id_value(&detail.name, &old_value)?),
            new: IssueStatusId::new(parse_id_value(&detail.name, &new_value)?),
        },
        "tracker_id" => JournalDetailAttr::TrackerId {
            old: TrackerId::new(parse_id_value(&detail.name, &old_value)?),
            new: TrackerId::new(parse_id_value(&detail.name, &new_value)?),
        },
        "project_id" => JournalDetailAttr::ProjectId {
            old: ProjectId::new(parse_id_value(&detail.name, &old_value)?),
            new: ProjectId::new(parse_id_value(&detail.name, &new_value)?),
        },
        "subject" => JournalDetailAttr::Subject {
            old: old_value,
            new: new_value,
        },
        "description" => JournalDetailAttr::Description {
            old: old_value,
            new: new_value,
        },
        "priority_id" => JournalDetailAttr::PriorityId {
            old: PriorityId::new(parse_id_value(&detail.name, &old_value)?),
            new: PriorityId::new(parse_id_value(&detail.name, &new_value)?),
        },
        "author_id" => JournalDetailAttr::AuthorId {
            old: UserId::new(parse_id_value(&detail.name, &old_value)?),
            new: UserId::new(parse_id_value(&detail.name, &new_value)?),
        },
        other => {
            return Err(RedmineClientError::Client {
                reason: format!("unsupported Redmine journal detail attribute '{other}'"),
            });
        }
    };

    Ok(Some(attr))
}

fn parse_id_value(name: &str, value: &str) -> Result<u16, RedmineClientError> {
    value
        .parse::<u16>()
        .map_err(|error| RedmineClientError::Client {
            reason: format!(
                "failed to parse Redmine journal detail attribute '{name}' value '{value}' as u16: {error}"
            ),
        })
}
