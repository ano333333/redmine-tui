use crate::clients::redmine::default::journal_detail_value_conversion::{
    parse_bool, parse_optional_calendar_date, parse_optional_u16, parse_optional_whole_hours,
    parse_required_u16, parse_timestamp,
};

#[test]
fn timestamp_accepts_rfc3339_offsets_and_fractional_seconds_as_the_same_instant() {
    let expected = parse_timestamp("2026-09-05T12:34:56.125Z").unwrap();

    for value in [
        "2026-09-05T12:34:56.125Z",
        "2026-09-05T21:34:56.125+09:00",
        "2026-09-05T08:34:56.125-04:00",
    ] {
        assert_eq!(parse_timestamp(value).unwrap(), expected);
    }
}

#[test]
fn timestamp_rejects_invalid_and_calendar_date_values() {
    for value in ["not-a-date", "2026-09-05"] {
        assert!(parse_timestamp(value).is_err());
    }
}

#[test]
fn optional_u16_maps_null_and_empty_to_none_and_parses_numbers() {
    assert_eq!(parse_optional_u16(None).unwrap(), None);
    assert_eq!(parse_optional_u16(Some("")).unwrap(), None);
    assert_eq!(parse_optional_u16(Some("42")).unwrap(), Some(42));
    assert_eq!(parse_optional_u16(Some("65535")).unwrap(), Some(65535));
    assert!(parse_optional_u16(Some("invalid")).is_err());
    assert!(parse_optional_u16(Some("65536")).is_err());
}

#[test]
fn required_u16_rejects_null_empty_and_invalid_values() {
    for value in [None, Some(""), Some("x"), Some("65536")] {
        assert!(parse_required_u16(value).is_err());
    }
    assert_eq!(parse_required_u16(Some("75")).unwrap(), 75);
}

#[test]
fn calendar_date_maps_null_and_empty_to_none_and_rejects_invalid_values() {
    assert_eq!(parse_optional_calendar_date(None).unwrap(), None);
    assert_eq!(parse_optional_calendar_date(Some("")).unwrap(), None);
    assert!(parse_optional_calendar_date(Some("2026-02-30")).is_err());
    assert!(parse_optional_calendar_date(Some("2026-9-5")).is_err());
    assert!(parse_optional_calendar_date(Some(" 2026-09-05")).is_err());
    assert!(parse_optional_calendar_date(Some("2026-09-05 ")).is_err());
    assert!(parse_optional_calendar_date(Some("2026-09-05T00:00:00Z")).is_err());
    assert!(parse_optional_calendar_date(Some("2026-09-05+09:00")).is_err());
    assert_eq!(
        parse_optional_calendar_date(Some("2026-09-05"))
            .unwrap()
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        "2026-09-05"
    );
}

#[test]
fn bool_accepts_redmine_encodings_and_rejects_other_values() {
    for (value, expected) in [("0", false), ("1", true), ("false", false), ("true", true)] {
        assert_eq!(parse_bool(Some(value)).unwrap(), expected);
    }
    for value in [None, Some(""), Some("2"), Some("TRUE")] {
        assert!(parse_bool(value).is_err());
    }
}

#[test]
fn whole_hours_accepts_integral_decimals_and_rejects_unrepresentable_values() {
    assert_eq!(parse_optional_whole_hours(None).unwrap(), None);
    assert_eq!(parse_optional_whole_hours(Some("")).unwrap(), None);
    assert_eq!(parse_optional_whole_hours(Some("8")).unwrap(), Some(8));
    assert_eq!(parse_optional_whole_hours(Some("8.0")).unwrap(), Some(8));

    for value in ["1.5", "NaN", "inf", "-1", "65536"] {
        assert!(parse_optional_whole_hours(Some(value)).is_err());
    }
}
