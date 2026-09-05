use chrono::{DateTime, Local, NaiveDate, TimeZone};

use crate::clients::redmine::RedmineClientError;

fn client_error(reason: impl Into<String>) -> RedmineClientError {
    RedmineClientError::Client {
        reason: reason.into(),
    }
}

/// Parse `value`. `value` must be in RFC 3339 format if not `None`.
pub(super) fn parse_timestamp(value: &str) -> Result<DateTime<Local>, RedmineClientError> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Local))
        .map_err(|error| {
            client_error(format!(
                "failed to parse Redmine datetime '{value}': {error}"
            ))
        })
}

pub(super) fn parse_required_u16(value: Option<&str>) -> Result<u16, RedmineClientError> {
    let value = value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| client_error("missing journal detail value"))?;
    value.parse().map_err(|error| {
        client_error(format!(
            "failed to parse journal detail integer '{value}': {error}"
        ))
    })
}

pub(super) fn parse_optional_u16(value: Option<&str>) -> Result<Option<u16>, RedmineClientError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| parse_required_u16(Some(value)))
        .transpose()
}

/// Parse `value`.`value` must be in "YYYY-MM-DD" format if not `None`.
pub(super) fn parse_optional_calendar_date(
    value: Option<&str>,
) -> Result<Option<DateTime<Local>>, RedmineClientError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let bytes = value.as_bytes();
    let has_calendar_date_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit());
    if !has_calendar_date_shape {
        return Err(client_error(format!(
            "failed to parse Redmine date '{value}': expected YYYY-MM-DD"
        )));
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        client_error(format!("failed to parse Redmine date '{value}': {error}"))
    })?;
    let datetime = date.and_hms_opt(0, 0, 0).ok_or_else(|| {
        client_error(format!(
            "failed to convert Redmine date '{value}' to datetime"
        ))
    })?;
    Local
        .from_local_datetime(&datetime)
        .single()
        .ok_or_else(|| {
            client_error(format!(
                "failed to convert Redmine date '{value}' to local datetime"
            ))
        })
        .map(Some)
}

pub(super) fn parse_bool(value: Option<&str>) -> Result<bool, RedmineClientError> {
    match value {
        Some("1" | "true") => Ok(true),
        Some("0" | "false") => Ok(false),
        _ => Err(client_error("invalid journal detail boolean")),
    }
}

pub(super) fn parse_optional_whole_hours(
    value: Option<&str>,
) -> Result<Option<u16>, RedmineClientError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let hours: f64 = value.parse().map_err(|error| {
        client_error(format!(
            "failed to parse journal detail hours '{value}': {error}"
        ))
    })?;
    if !hours.is_finite() || hours.fract() != 0.0 || !(0.0..=u16::MAX as f64).contains(&hours) {
        return Err(client_error(format!(
            "journal detail hours '{value}' is not a whole u16 value"
        )));
    }
    Ok(Some(hours as u16))
}
