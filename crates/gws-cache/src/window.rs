use crate::{Error, Result};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

/// `all`, a fixed duration (30-day months, 365-day years), or a UTC date.
#[derive(Clone, Copy, Debug, Default)]
pub enum Window {
    #[default]
    Unbounded,
    Relative(TimeDelta),
    Absolute(DateTime<Utc>),
}
impl std::str::FromStr for Window {
    type Err = Error;
    fn from_str(raw: &str) -> Result<Self> {
        let expr = raw.trim();
        if expr.is_empty() || expr.eq_ignore_ascii_case("all") {
            return Ok(Self::Unbounded);
        }
        if let Ok(date) = NaiveDate::parse_from_str(expr, "%Y-%m-%d") {
            return Ok(Self::Absolute(
                date.and_hms_opt(0, 0, 0).expect("midnight").and_utc(),
            ));
        }
        let invalid = || {
            Error::Invalid(format!(
                "invalid window {raw:?}; use 4w, 30d, YYYY-MM-DD or all"
            ))
        };
        let split = expr.char_indices().last().ok_or_else(invalid)?.0;
        let (number, unit) = expr.split_at(split);
        let quantity: i64 = number.parse().map_err(|_| invalid())?;
        if quantity < 0 {
            return Err(invalid());
        }
        let seconds = match unit {
            "h" => 3600,
            "d" => 86400,
            "w" => 604800,
            "m" => 2592000,
            "y" => 31536000,
            _ => return Err(invalid()),
        };
        let duration = quantity
            .checked_mul(seconds)
            .and_then(TimeDelta::try_seconds)
            .ok_or_else(invalid)?;
        Ok(Self::Relative(duration))
    }
}
impl Window {
    pub fn start(self, now: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
        self.bound(now, false)
    }
    pub fn end(self, now: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
        self.bound(now, true)
    }
    fn bound(self, now: DateTime<Utc>, forward: bool) -> Result<Option<DateTime<Utc>>> {
        match self {
            Self::Unbounded => Ok(None),
            Self::Absolute(at) => Ok(Some(at)),
            Self::Relative(duration) => if forward {
                now.checked_add_signed(duration)
            } else {
                now.checked_sub_signed(duration)
            }
            .map(Some)
            .ok_or_else(|| Error::Invalid("window exceeds the supported calendar range".into())),
        }
    }
}
