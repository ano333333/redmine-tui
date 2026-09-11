//! Redmineの日付・日時文字列をドメインで扱うローカル日時へ変換する。

use chrono::{DateTime, Local, NaiveDate, TimeZone};

use crate::clients::redmine::RedmineClientError;

/// RFC 3339形式の日時を、時点を保ったままローカル時刻へ変換する。
pub(super) fn parse_datetime(value: &str) -> Result<DateTime<Local>, RedmineClientError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Local))
        .map_err(|error| RedmineClientError::Client {
            reason: format!("failed to parse Redmine datetime '{value}': {error}"),
        })
}

/// Redmineの日付が未指定または空文字なら欠損として扱い、それ以外はローカル時刻の午前0時として解釈する。
pub(super) fn parse_optional_date(
    value: Option<String>,
) -> Result<Option<DateTime<Local>>, RedmineClientError> {
    let Some(value) = value else {
        return Ok(None);
    };

    if value.is_empty() {
        return Ok(None);
    }

    let date = NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
        RedmineClientError::Client {
            reason: format!("failed to parse Redmine date '{value}': {error}"),
        }
    })?;
    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| RedmineClientError::Client {
            reason: format!("failed to convert Redmine date '{value}' to datetime"),
        })?;

    Local
        .from_local_datetime(&datetime)
        .single()
        .ok_or_else(|| RedmineClientError::Client {
            reason: format!("failed to convert Redmine date '{value}' to local datetime"),
        })
        .map(Some)
}
