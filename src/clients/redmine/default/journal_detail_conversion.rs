//! RedmineのJournal detail JSONで表されるIssue属性をdomain型へ変換する。

use serde::Deserialize;

use crate::clients::redmine::RedmineClientError;
use crate::vos::{IssueStatusId, JournalDetailAttr, PriorityId, ProjectId, TrackerId, UserId};

use super::value_conversion;

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
/// [`RedmineClientError::Client`]を返す。日付形式が不正な場合、予定工数が整数でないか
/// `u16`の範囲外の場合、非公開フラグが`0` / `1` / `false` / `true`以外の場合、
/// 進捗率の値が欠落している場合も同じエラーを返す。
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
        // 未設定を表す欠損値と空文字列は、Optional ID属性ではどちらも`None`として扱う。
        "category_id" => JournalDetailAttr::CategoryId {
            old: parse_optional_id_value(&detail.name, Some(old_value.as_str()))?,
            new: parse_optional_id_value(&detail.name, Some(new_value.as_str()))?,
        },
        "assigned_to_id" => JournalDetailAttr::AssignedToId {
            old: parse_optional_id_value(&detail.name, Some(old_value.as_str()))?,
            new: parse_optional_id_value(&detail.name, Some(new_value.as_str()))?,
        },
        "fixed_version_id" => JournalDetailAttr::FixedVersionId {
            old: parse_optional_id_value(&detail.name, Some(old_value.as_str()))?,
            new: parse_optional_id_value(&detail.name, Some(new_value.as_str()))?,
        },
        // 日付属性は他のRedmine DTOと同じ欠損値・日付形式の契約で変換する。
        "start_date" => JournalDetailAttr::StartDate {
            old: value_conversion::parse_optional_date(Some(old_value))?,
            new: value_conversion::parse_optional_date(Some(new_value))?,
        },
        "due_date" => JournalDetailAttr::DueDate {
            old: value_conversion::parse_optional_date(Some(old_value))?,
            new: value_conversion::parse_optional_date(Some(new_value))?,
        },
        "done_ratio" => JournalDetailAttr::DoneRatio {
            // 進捗率はOptional属性ではないため、欠損値を`0`として受け入れない。
            old: parse_id_value(&detail.name, &old_value)?,
            new: parse_id_value(&detail.name, &new_value)?,
        },
        "estimated_hours" => JournalDetailAttr::EstimatedHours {
            // Redmineの小数表現は受け取るが、domainが想定する整数時間だけに制限する。
            old: parse_optional_whole_hours(&detail.name, Some(old_value.as_str()))?,
            new: parse_optional_whole_hours(&detail.name, Some(new_value.as_str()))?,
        },
        "parent_id" => JournalDetailAttr::ParentId {
            // 親Issueの解除は、他のOptional ID属性と同様に欠損値または空文字列で表される。
            old: parse_optional_id_value(&detail.name, Some(old_value.as_str()))?,
            new: parse_optional_id_value(&detail.name, Some(new_value.as_str()))?,
        },
        "is_private" => JournalDetailAttr::IsPrivate {
            // Redmineが返す数値表現と真偽値表現の4種類のみを有効とする。
            old: parse_bool(&detail.name, &old_value)?,
            new: parse_bool(&detail.name, &new_value)?,
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

/// Optional IDを変換し、欠損値または空文字列は`None`として扱う。
///
/// 数値検証を必須IDと共通化しつつ、IDごとのdomain型へ変換する。
fn parse_optional_id_value<T>(
    name: &str,
    value: Option<&str>,
) -> Result<Option<T>, RedmineClientError>
where
    T: From<u16>,
{
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }

    parse_id_value(name, value).map(|id| T::from(id)).map(Some)
}

/// Redmineの小数表現から整数時間だけを変換し、欠損値または空文字列は`None`として扱う。
fn parse_optional_whole_hours(
    name: &str,
    value: Option<&str>,
) -> Result<Option<u16>, RedmineClientError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }

    let hours = value.parse::<f64>().map_err(|error| RedmineClientError::Client {
        reason: format!(
            "failed to parse Redmine journal detail attribute '{name}' value '{value}' as f64: {error}"
        ),
    })?;
    if !(0.0..=u16::MAX as f64).contains(&hours) || hours.fract() != 0.0 {
        return Err(RedmineClientError::Client {
            reason: format!(
                "failed to convert Redmine journal detail attribute '{name}' value '{value}' to whole u16"
            ),
        });
    }

    Ok(Some(hours as u16))
}

/// Redmineの真偽値表現のうち`0` / `1` / `false` / `true`だけを受け入れる。
fn parse_bool(name: &str, value: &str) -> Result<bool, RedmineClientError> {
    match value {
        "0" | "false" => Ok(false),
        "1" | "true" => Ok(true),
        _ => Err(RedmineClientError::Client {
            reason: format!(
                "failed to parse Redmine journal detail attribute '{name}' value '{value}' as bool"
            ),
        }),
    }
}
