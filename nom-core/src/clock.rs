//! Timezone-aware Clock for "today" resolution and date materialization.
//!
//! Resolves timezone once at startup from config (explicit IANA string) or
//! system-local fallback. Computes "today" fresh on every call — never cached.
//! Used by Operation execution and write-time logged_date materialization.

use crate::config::AppConfig;
use crate::error::ErrorData;
use chrono::{DateTime, NaiveDate, Utc};

/// A timezone-aware clock that computes dates fresh on every call.
///
/// Holds a resolved `chrono_tz::Tz` (owned, cheap to copy). All date
/// computations flow through UTC → local TZ conversion, avoiding DST
/// ambiguity by construction.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub(crate) tz: chrono_tz::Tz,
}

impl Clock {
    /// Resolve timezone from config, falling back to system-local then UTC.
    ///
    /// Resolution order:
    /// 1. If `config.timezone` is `Some(tz_string)`: parse as IANA timezone.
    ///    On failure, return a validation error.
    /// 2. If unset: call `iana_time_zone::get_timezone()` for OS-local TZ.
    ///    If available, parse it same as above.
    /// 3. Last resort: fall back to UTC with a warning.
    pub fn new(config: &AppConfig) -> Result<Self, ErrorData> {
        let tz = match &config.timezone {
            Some(tz_str) => Self::parse_tz(tz_str)?,
            None => Self::resolve_os_tz(iana_time_zone::get_timezone()),
        };
        Ok(Self { tz })
    }

    fn parse_tz(s: &str) -> Result<chrono_tz::Tz, ErrorData> {
        s.parse::<chrono_tz::Tz>()
            .map_err(|_| ErrorData::validation("timezone", format!("invalid IANA timezone: {s}")))
    }

    /// Resolve the OS-local timezone, falling back to UTC (with a warning)
    /// whenever detection fails outright OR the OS reports a string that
    /// isn't a valid IANA zone — neither case is a user config error, so
    /// unlike `parse_tz` above neither should fail startup.
    fn resolve_os_tz(os_tz: Result<String, iana_time_zone::GetTimezoneError>) -> chrono_tz::Tz {
        match os_tz.ok().and_then(|s| s.parse::<chrono_tz::Tz>().ok()) {
            Some(tz) => tz,
            None => {
                tracing::warn!("no usable OS timezone detected, using UTC");
                chrono_tz::UTC
            }
        }
    }

    /// Compute today's date in the configured timezone.
    ///
    /// Zero-cache: computed fresh every call from `Utc::now()`.
    /// DST-safe because UTC → local conversion always produces a valid date.
    pub fn today(&self) -> NaiveDate {
        Utc::now().with_timezone(&self.tz).date_naive()
    }

    /// Materialize `logged_date` from a UTC timestamp at write time.
    ///
    /// Converts the given UTC datetime to local date. Historical values are
    /// never retroactively recomputed — this is only called at write time.
    pub fn logged_date(&self, utc_datetime: &DateTime<Utc>) -> NaiveDate {
        utc_datetime.with_timezone(&self.tz).date_naive()
    }

    /// Format a `NaiveDate` as `"YYYY-MM-DD"` for SQLite storage.
    pub fn format_date(date: NaiveDate) -> String {
        date.format("%Y-%m-%d").to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_today_returns_reasonable_date() {
        let clock = Clock { tz: chrono_tz::UTC };
        let today = clock.today();
        let now = Utc::now().date_naive();
        // Should be within ±1 day of current UTC date (handles edge cases near midnight)
        let diff = (today - now).num_days().abs();
        assert!(
            diff <= 1,
            "today should be close to current date: {} vs {}",
            today,
            now
        );
    }

    #[test]
    fn test_clock_logged_date_materializes_correctly() {
        // Parse timezone from string — avoids fragile enum variant names
        let tz: chrono_tz::Tz = "America/New_York".parse().unwrap();
        let clock = Clock { tz };

        // 2024-06-15T03:00 UTC = 2024-06-14 23:00 in America/New_York (EDT, UTC-4)
        let utc_dt = DateTime::parse_from_rfc3339("2024-06-15T03:00:00Z")
            .unwrap()
            .into();
        let logged = clock.logged_date(&utc_dt);
        assert_eq!(logged, NaiveDate::from_ymd_opt(2024, 6, 14).unwrap());
    }

    #[test]
    fn test_clock_format_date() {
        let date = NaiveDate::from_ymd_opt(2024, 8, 15).unwrap();
        assert_eq!(Clock::format_date(date), "2024-08-15");
    }

    #[test]
    fn test_clock_new_with_explicit_timezone() {
        let mut config = AppConfig::load().expect("config load");
        config.timezone = Some("America/New_York".to_string());
        let clock = Clock::new(&config).expect("should parse valid timezone");
        // Verify it parsed correctly by checking the debug name contains New_York
        let debug = format!("{:?}", clock.tz);
        assert!(
            debug.contains("New_York"),
            "tz should be America/New_York, got {}",
            debug
        );
    }

    #[test]
    fn test_clock_new_with_invalid_timezone() {
        let mut config = AppConfig::load().expect("config load");
        config.timezone = Some("Invalid/Zone".to_string());
        let result = Clock::new(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, crate::error::ErrorCategory::Validation);
        assert_eq!(err.field.as_deref(), Some("timezone"));
    }

    #[test]
    fn test_clock_new_fallback_to_utc() {
        let config = AppConfig::load().expect("config load");
        // When timezone is None and OS detection may or may not work,
        // we should get a valid Clock (either OS TZ or UTC)
        let _clock = Clock::new(&config).expect("should create clock with fallback");
    }

    #[test]
    fn test_resolve_os_tz_uses_valid_os_string() {
        let tz = Clock::resolve_os_tz(Ok("America/New_York".to_string()));
        assert_eq!(tz, "America/New_York".parse::<chrono_tz::Tz>().unwrap());
    }

    #[test]
    fn test_resolve_os_tz_falls_back_to_utc_on_unparseable_string() {
        // The OS successfully reported a string, but it isn't a valid IANA
        // zone — this must fall back to UTC, not fail Clock::new() startup.
        let tz = Clock::resolve_os_tz(Ok("Not/AValidZone".to_string()));
        assert_eq!(tz, chrono_tz::UTC);
    }

    #[test]
    fn test_resolve_os_tz_falls_back_to_utc_on_detection_failure() {
        let tz = Clock::resolve_os_tz(Err(iana_time_zone::GetTimezoneError::OsError));
        assert_eq!(tz, chrono_tz::UTC);
    }
}
