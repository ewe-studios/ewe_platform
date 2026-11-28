//! The retry policy of the Durable Streams client.
//!
//! The policy decides when a request is safe to repeat, how long to wait, and
//! when to stop. It reads the clock and the random number generator from the
//! [`Ctx`] the client carries, so a deterministic simulation test replays the
//! same delays.
//!
//! A caller repeats an operation by passing a closure to
//! [`RetryPolicy::execute`]. The closure receives the one-based attempt number
//! and builds the request again, so it evaluates dynamic headers once per
//! attempt.

use super::core::ClientError;
use foundation_compact::rng::Rng;
use foundation_context::{Ctx, Duration, Instant};
use std::collections::BTreeMap;

/// The default number of attempts, including the first one.
///
/// Three attempts let a read survive two transient failures, which the
/// reference client suite injects.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;
/// The default delay before the first retry.
pub const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(50);
/// The default cap for the exponential backoff.
pub const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(2);
/// The default backoff multiplier for each retry.
pub const DEFAULT_MULTIPLIER: f64 = 2.0;
/// The default fraction of a delay that jitter removes.
pub const DEFAULT_JITTER: f64 = 0.2;
/// The default cap for a server `Retry-After` value.
///
/// A test server must not stop a run for longer than this. Confirm the value
/// against what a browser or a gateway waits before it gives up.
pub const DEFAULT_RETRY_AFTER_CAP: Duration = Duration::from_secs(2);

/// The statuses that the client retries by default.
///
/// The reference suite injects `500`, `503` and `429`. `502` and `504` name a
/// failed upstream and a slow upstream. The list matches feature 07 requirement
/// 11.
pub const DEFAULT_RETRY_STATUSES: [u16; 5] = [429, 500, 502, 503, 504];

/// When to repeat a request, and how long to wait.
///
/// Every limit is a field, so a caller can tune it. The [`Default`]
/// implementation documents the defaults.
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    /// The maximum number of attempts, including the first one. `0` is treated
    /// as `1`, so the request runs once. Default: [`DEFAULT_MAX_ATTEMPTS`].
    pub max_attempts: u32,
    /// The delay before the first retry. Default:
    /// [`DEFAULT_INITIAL_BACKOFF`].
    pub initial_backoff: Duration,
    /// The cap for the exponential backoff. Default: [`DEFAULT_MAX_BACKOFF`].
    pub max_backoff: Duration,
    /// The factor applied to the backoff after each retry. Default:
    /// [`DEFAULT_MULTIPLIER`].
    pub multiplier: f64,
    /// The fraction of a delay that jitter removes, in `0.0..=1.0`. Default:
    /// [`DEFAULT_JITTER`].
    pub jitter: f64,
    /// The cap for a server `Retry-After` value. Default:
    /// [`DEFAULT_RETRY_AFTER_CAP`].
    pub retry_after_cap: Duration,
    /// The statuses that the client retries. Default:
    /// [`DEFAULT_RETRY_STATUSES`].
    pub retry_statuses: Vec<u16>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            multiplier: DEFAULT_MULTIPLIER,
            jitter: DEFAULT_JITTER,
            retry_after_cap: DEFAULT_RETRY_AFTER_CAP,
            retry_statuses: DEFAULT_RETRY_STATUSES.to_vec(),
        }
    }
}

impl RetryPolicy {
    /// Build a policy with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of attempts, including the first one.
    #[must_use]
    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set the delay before the first retry.
    #[must_use]
    pub fn initial_backoff(mut self, delay: Duration) -> Self {
        self.initial_backoff = delay;
        self
    }

    /// Set the cap for the exponential backoff.
    #[must_use]
    pub fn max_backoff(mut self, delay: Duration) -> Self {
        self.max_backoff = delay;
        self
    }

    /// Set the backoff multiplier.
    #[must_use]
    pub fn multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Set the jitter fraction, in `0.0..=1.0`.
    #[must_use]
    pub fn jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter;
        self
    }

    /// Set the cap for a server `Retry-After` value.
    #[must_use]
    pub fn retry_after_cap(mut self, delay: Duration) -> Self {
        self.retry_after_cap = delay;
        self
    }

    /// Replace the retried status list.
    #[must_use]
    pub fn retry_statuses(mut self, statuses: Vec<u16>) -> Self {
        self.retry_statuses = statuses;
        self
    }

    /// True when this policy repeats the given failure.
    ///
    /// A transport failure and a listed status are transient. Every other
    /// failure is permanent: a `404`, a `409` conflict and a malformed response
    /// never repeat.
    #[must_use]
    pub fn should_retry(&self, error: &ClientError) -> bool {
        match error {
            ClientError::Transport(_) => true,
            ClientError::UnexpectedStatus { status, .. } => self.retry_statuses.contains(status),
            _ => false,
        }
    }

    /// The number of attempts this policy allows. It is never zero.
    #[must_use]
    pub fn attempt_limit(&self) -> u32 {
        self.max_attempts.max(1)
    }

    /// The exponential backoff for retry number `attempt`, with jitter.
    ///
    /// `attempt` counts retries after the first request: `1` is the wait before
    /// the second attempt. The delay is `initial_backoff` times
    /// `multiplier` raised to `attempt` minus one, capped at `max_backoff`,
    /// then reduced by a random part of at most `jitter`. The random part comes
    /// from `ctx.rng()`, so a fixed seed gives a fixed delay.
    #[must_use]
    pub fn backoff(&self, attempt: u32, ctx: Option<&Ctx>) -> Duration {
        let exponent = i32::try_from(attempt.saturating_sub(1)).unwrap_or(i32::MAX);
        let multiplier = if self.multiplier.is_finite() {
            self.multiplier.max(1.0)
        } else {
            1.0
        };
        let mut seconds = self.initial_backoff.as_secs_f64() * multiplier.powi(exponent);
        let max_seconds = self.max_backoff.as_secs_f64();
        if !seconds.is_finite() || seconds > max_seconds {
            seconds = max_seconds;
        }
        let base = Duration::from_secs_f64(seconds);
        self.with_jitter(base, ctx)
    }

    /// The delay before retry number `attempt`.
    ///
    /// A `Retry-After` header wins over the backoff, capped at
    /// `retry_after_cap`. The header holds either a count of seconds or a
    /// Hypertext Transfer Protocol (HTTP) date. The date is compared with the
    /// wall clock of `ctx`.
    #[must_use]
    pub fn delay(&self, error: &ClientError, attempt: u32, ctx: Option<&Ctx>) -> Duration {
        if let Some(after) = retry_after(error, ctx) {
            return after.min(self.retry_after_cap);
        }
        self.backoff(attempt, ctx)
    }

    /// Run `attempt` until it succeeds, a failure is permanent, or the policy
    /// runs out of attempts.
    ///
    /// The closure receives the one-based attempt number. It runs again for
    /// each retry and builds the request from scratch, so a dynamic header
    /// provider is evaluated once per attempt.
    ///
    /// A client with no [`Ctx`] does not retry: a wait needs an injected clock,
    /// and the policy never reads the real one. The first failure returns
    /// unchanged. Call [`super::DurableClient::with_ctx`] to enable retry.
    ///
    /// # Errors
    ///
    /// Returns the permanent failure unchanged. Returns
    /// [`ClientError::RetryExhausted`] when the last allowed attempt also
    /// failed. Returns [`ClientError::MissingClock`] when the [`Ctx`] has no
    /// timer, as on a wasm host that installed no environment.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(max_attempts = self.attempt_limit()),
        err
    )]
    pub async fn execute<T, F, Fut>(
        &self,
        ctx: Option<&Ctx>,
        mut attempt: F,
    ) -> Result<T, ClientError>
    where
        F: FnMut(u32) -> Fut,
        Fut: core::future::Future<Output = Result<T, ClientError>>,
    {
        let Some(ctx) = ctx else {
            return attempt(1).await;
        };
        let limit = self.attempt_limit();
        let mut number = 0_u32;
        loop {
            number += 1;
            match attempt(number).await {
                Ok(value) => return Ok(value),
                Err(error) => {
                    if !self.should_retry(&error) {
                        return Err(error);
                    }
                    if number >= limit {
                        return Err(exhausted(error, number));
                    }
                    let delay = self.delay(&error, number, Some(ctx));
                    wait(ctx, delay).await?;
                }
            }
        }
    }

    /// Remove a random part of `base`, bounded by the jitter fraction.
    fn with_jitter(&self, base: Duration, ctx: Option<&Ctx>) -> Duration {
        let jitter = self.jitter.clamp(0.0, 1.0);
        if jitter == 0.0 || base.is_zero() {
            return base;
        }
        let Some(ctx) = ctx else {
            return base;
        };
        let rng = ctx.rng();
        let mut rng = rng.lock().expect("the context RNG lock is not poisoned");
        // The low 32 bits are uniform enough for a jitter fraction.
        let sample = u32::try_from(rng.next_u64() & u64::from(u32::MAX)).unwrap_or(0);
        let unit = f64::from(sample) / f64::from(u32::MAX);
        let reduction = base.mul_f64(jitter * unit);
        base.saturating_sub(reduction)
    }
}

/// Build the typed error for a policy that ran out of attempts.
fn exhausted(error: ClientError, attempts: u32) -> ClientError {
    match error {
        ClientError::UnexpectedStatus {
            status,
            body,
            headers,
        } => ClientError::RetryExhausted {
            attempts,
            status: Some(status),
            body,
            headers,
        },
        ClientError::Transport(message) => ClientError::RetryExhausted {
            attempts,
            status: None,
            body: message,
            headers: BTreeMap::new(),
        },
        other => other,
    }
}

/// Wait until `delay` passes on the clock of `ctx`.
async fn wait(ctx: &Ctx, delay: Duration) -> Result<(), ClientError> {
    if delay.is_zero() {
        return Ok(());
    }
    let timer = ctx
        .try_timer()
        .map_err(|error| ClientError::MissingClock(error.to_string()))?;
    let deadline: Instant = ctx.clock().now() + delay;
    timer.sleep_until(ctx.clone(), deadline).await;
    Ok(())
}

/// The `Retry-After` delay of a status failure, when the header is present.
fn retry_after(error: &ClientError, ctx: Option<&Ctx>) -> Option<Duration> {
    let ClientError::UnexpectedStatus { headers, .. } = error else {
        return None;
    };
    let value = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .map(|(_, value)| value.trim())?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let target = parse_http_date(value)?;
    let now = ctx?.clock().wall_now();
    let now_seconds = foundation_compact::time::unix_seconds(now);
    let remaining = target.saturating_sub(u64::try_from(now_seconds).unwrap_or(0));
    Some(Duration::from_secs(remaining))
}

/// Parse one HTTP date into whole seconds since the UNIX epoch.
///
/// It accepts the three formats of Hypertext Transfer Protocol (HTTP) section
/// 7.1.1.1: the preferred `IMF-fixdate` form (`Sun, 06 Nov 1994 08:49:37 GMT`),
/// the obsolete `RFC 850` form (`Sunday, 06-Nov-94 08:49:37 GMT`) and the
/// `asctime` form (`Sun Nov  6 08:49:37 1994`).
fn parse_http_date(value: &str) -> Option<u64> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let (date, time) = match tokens.as_slice() {
        // IMF-fixdate: `Sun, 06 Nov 1994 08:49:37 GMT`.
        [_weekday, day, month, year, time, zone] if zone.eq_ignore_ascii_case("GMT") => {
            let day = parse_small(day)?;
            let month = parse_month(month)?;
            let year = parse_small(year)?;
            ((year, month, day), *time)
        }
        // Obsolete RFC 850: `Sunday, 06-Nov-94 08:49:37 GMT`.
        [_weekday, date, time, zone] if zone.eq_ignore_ascii_case("GMT") => {
            (parse_rfc850_date(date)?, *time)
        }
        // asctime: `Sun Nov  6 08:49:37 1994`.
        [_weekday, month, day, time, year] => {
            let month = parse_month(month)?;
            let day = parse_small(day)?;
            let year = parse_small(year)?;
            ((year, month, day), *time)
        }
        _ => return None,
    };
    let (year, month, day) = date;
    let (hour, minute, second) = parse_clock(time)?;
    let days = days_from_civil(year, month, day);
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(hour * 3_600 + minute * 60 + second)?;
    u64::try_from(seconds).ok()
}

/// Parse the date field of an obsolete `RFC 850` value: `06-Nov-94`.
fn parse_rfc850_date(date: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = date.split('-').collect();
    let [day, month, year] = parts.as_slice() else {
        return None;
    };
    let day = parse_small(day)?;
    let month = parse_month(month)?;
    let year = parse_small(year)?;
    // A two-digit year maps into 1970..=2069, as HTTP section 7.1.1.1 says.
    let year = if year < 70 { year + 2000 } else { year + 1900 };
    Some((year, month, day))
}

/// Parse a small decimal number, such as a day or a year.
fn parse_small(value: &str) -> Option<i64> {
    value.trim_matches(',').parse::<i64>().ok()
}

/// Parse a clock field: `08:49:37`.
fn parse_clock(value: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = value.split(':').collect();
    let [hour, minute, second] = parts.as_slice() else {
        return None;
    };
    let hour = hour.parse::<i64>().ok()?;
    let minute = minute.parse::<i64>().ok()?;
    let second = second.parse::<i64>().ok()?;
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some((hour, minute, second))
}

/// Parse one three-letter month name into `1..=12`.
fn parse_month(value: &str) -> Option<i64> {
    let name = value.trim_matches(',');
    let month = match name {
        name if name.eq_ignore_ascii_case("Jan") => 1,
        name if name.eq_ignore_ascii_case("Feb") => 2,
        name if name.eq_ignore_ascii_case("Mar") => 3,
        name if name.eq_ignore_ascii_case("Apr") => 4,
        name if name.eq_ignore_ascii_case("May") => 5,
        name if name.eq_ignore_ascii_case("Jun") => 6,
        name if name.eq_ignore_ascii_case("Jul") => 7,
        name if name.eq_ignore_ascii_case("Aug") => 8,
        name if name.eq_ignore_ascii_case("Sep") => 9,
        name if name.eq_ignore_ascii_case("Oct") => 10,
        name if name.eq_ignore_ascii_case("Nov") => 11,
        name if name.eq_ignore_ascii_case("Dec") => 12,
        _ => return None,
    };
    Some(month)
}

/// The number of days from 1970-01-01 to `year-month-day`.
///
/// This is the civil-from-days algorithm of Howard Hinnant, with the epoch
/// shifted to the UNIX epoch.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
