use chrono::{DateTime, FixedOffset};
use serde_json::{Value, json};

use super::numeric::json_number;

fn parse_datetime(value: &Value) -> Option<DateTime<FixedOffset>> {
    let s = value.as_str()?;
    DateTime::parse_from_rfc3339(s).ok().or_else(|| {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().fixed_offset())
    })
}

pub(super) fn date_part(args: &[Value], part: fn(&DateTime<FixedOffset>) -> i64) -> Value {
    parse_datetime(&args[0]).map_or(Value::Null, |d| json!(part(&d)))
}

pub(super) fn fn_now(_args: &[Value]) -> Value {
    json!(chrono::Utc::now().to_rfc3339())
}

pub(super) fn fn_date_diff(args: &[Value]) -> Value {
    let (Some(from), Some(to), Some(unit)) = (
        parse_datetime(&args[0]),
        parse_datetime(&args[1]),
        args[2].as_str(),
    ) else {
        return Value::Null;
    };
    let seconds = (to - from).num_milliseconds() as f64 / 1000.0;
    let factor = match unit.to_ascii_lowercase().as_str() {
        "days" => 86_400.0,
        "hours" => 3_600.0,
        "minutes" => 60.0,
        "seconds" => 1.0,
        _ => return Value::Null,
    };
    json_number(seconds / factor)
}

/// `DATE_ADD(date, amount, unit)` → RFC 3339 string.
///
/// Units match `DATE_DIFF` exactly (days/hours/minutes/seconds) so the two are
/// inverses. Calendar units (months, years) are deliberately absent: they are
/// not a fixed number of seconds, so "+1 month" needs a documented convention
/// for month-end that this registry has no way to express yet.
pub(super) fn fn_date_add(args: &[Value]) -> Value {
    let (Some(base), Some(amount), Some(unit)) =
        (parse_datetime(&args[0]), args[1].as_f64(), args[2].as_str())
    else {
        return Value::Null;
    };
    let seconds = match unit.to_ascii_lowercase().as_str() {
        "days" => amount * 86_400.0,
        "hours" => amount * 3_600.0,
        "minutes" => amount * 60.0,
        "seconds" => amount,
        _ => return Value::Null,
    };
    if !seconds.is_finite() {
        return Value::Null;
    }
    let delta = chrono::Duration::milliseconds((seconds * 1000.0).round() as i64);
    match base.checked_add_signed(delta) {
        Some(shifted) => json!(shifted.to_rfc3339()),
        None => Value::Null,
    }
}
