use chrono::{DateTime, FixedOffset, Local, SecondsFormat};
use miette::Result;

use crate::constants::DEFAULT_TZ;

pub(crate) fn now(timezone: &str) -> Result<DateTime<FixedOffset>> {
    if !matches!(timezone, DEFAULT_TZ | "US/Central" | "CST6CDT") {
        tracing::warn!(
            timezone,
            "falling back to local offset; named timezone support is intentionally narrow"
        );
    }
    Ok(Local::now().fixed_offset())
}

pub(crate) fn iso(value: &DateTime<FixedOffset>) -> String {
    value.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}
