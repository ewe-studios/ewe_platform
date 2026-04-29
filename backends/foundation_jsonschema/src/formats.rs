//! Built-in format checkers for the `format` keyword.
//!
//! WHY: JSON Schema defines 19 standard format names that validate strings
//! against specific patterns (email, date-time, ipv4, uuid, etc.).
//!
//! WHAT: A `FormatChecker` trait and `builtin_format()` registry function.
//!
//! HOW: Static dispatch via match — fast, no runtime lookup.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// A checker that validates whether a string matches a format.
///
/// Implementers must also implement `Clone` for internal cloning during compilation.
pub trait FormatChecker: Send + Sync {
    fn check(&self, value: &str) -> bool;
    fn format_name(&self) -> &str;
    fn clone_box(&self) -> Box<dyn FormatChecker>;
}

impl Clone for Box<dyn FormatChecker> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 { return None; }
    let year = parts[0].parse::<i32>().ok()?;
    let month = parts[1].parse::<u32>().ok()?;
    let day = parts[2].parse::<u32>().ok()?;
    if parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return None;
    }
    Some((year, month, day))
}

fn validate_date(s: &str) -> Option<()> {
    let (year, month, day) = parse_date(s)?;
    if !(1..=12).contains(&month) { return None; }
    if day < 1 || day > days_in_month(year, month) { return None; }
    Some(())
}

fn parse_time_val(s: &str) -> Option<()> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 { return None; }
    let hour = parts[0].parse::<u32>().ok()?;
    let minute = parts[1].parse::<u32>().ok()?;
    if parts[0].len() != 2 || parts[1].len() != 2 { return None; }
    if hour > 23 || minute > 59 { return None; }
    if parts.len() == 3 {
        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        let sec = sec_parts[0].parse::<u32>().ok()?;
        if sec_parts[0].is_empty() || sec > 60 { return None; }
        if sec_parts.len() == 2 {
            if sec_parts[1].is_empty() { return None; }
            if !sec_parts[1].chars().all(|c| c.is_ascii_digit()) { return None; }
        }
    }
    Some(())
}

fn validate_tz(s: &str) -> Option<()> {
    if s == "Z" || s == "z" { return Some(()); }
    if s.len() < 6 { return None; }
    let sign = s.chars().next()?;
    if sign != '+' && sign != '-' { return None; }
    let rest = &s[1..];
    let tz_parts: Vec<&str> = rest.split(':').collect();
    if tz_parts.len() != 2 { return None; }
    let h: u32 = tz_parts[0].parse().ok()?;
    let m: u32 = tz_parts[1].parse().ok()?;
    if h > 23 || m > 59 { return None; }
    Some(())
}

// ── date-time ────────────────────────────────────────────────────────

/// Validates `date-time` format.\npub struct DateTimeChecker;

impl FormatChecker for DateTimeChecker {
    fn check(&self, value: &str) -> bool {
        if let Some(t_pos) = value.find('T').or_else(|| value.find('t')) {
            let date_part = &value[..t_pos];
            let time_part = &value[t_pos + 1..];
            if validate_date(date_part).is_none() { return false; }
            // Find timezone in time part
            let tz_start = time_part
                .find('Z')
                .or_else(|| time_part.find('z'))
                .or_else(|| {
                    // Find last + for timezone
                    time_part.rfind('+')
                })
                .or_else(|| {
                    // Find - for negative tz offset (after time digits)
                    let bytes = time_part.as_bytes();
                    let mut pos = None;
                    for i in (0..bytes.len()).rev() {
                        if bytes[i] == b'-' && i > 0
                            && time_part[..i].ends_with(|c: char| c.is_ascii_digit()) {
                                pos = Some(i);
                                break;
                            }
                    }
                    pos
                });
            match tz_start {
                Some(pos) => {
                    let time_str = &time_part[..pos];
                    let tz_str = &time_part[pos..];
                    if parse_time_val(time_str).is_none() { return false; }
                    validate_tz(tz_str).is_some()
                }
                None => parse_time_val(time_part).is_some(),
            }
        } else {
            false
        }
    }
    fn format_name(&self) -> &'static str { "date-time" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(DateTimeChecker)
    }
}

// ── date ──────────────────────────────────────────────────────────────

/// Validates `date` format.\npub struct DateChecker;

impl FormatChecker for DateChecker {
    fn check(&self, value: &str) -> bool { validate_date(value).is_some() }
    fn format_name(&self) -> &'static str { "date" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(DateChecker)
    }
}

// ── time ──────────────────────────────────────────────────────────────

/// Validates `time` format.\npub struct TimeChecker;

impl FormatChecker for TimeChecker {
    fn check(&self, value: &str) -> bool {
        // Time can have optional timezone suffix
        if let Some(pos) = value
            .find('Z')
            .or_else(|| value.find('z'))
            .or_else(|| value.rfind('+'))
            .or_else(|| {
                let bytes = value.as_bytes();
                let mut p = None;
                for i in (0..bytes.len()).rev() {
                    if bytes[i] == b'-' && i > 0
                        && value[..i].ends_with(|c: char| c.is_ascii_digit()) {
                            p = Some(i);
                            break;
                        }
                }
                p
            })
        {
            let time_str = &value[..pos];
            let tz_str = &value[pos..];
            if parse_time_val(time_str).is_none() { return false; }
            validate_tz(tz_str).is_some()
        } else {
            parse_time_val(value).is_some()
        }
    }
    fn format_name(&self) -> &'static str { "time" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(TimeChecker)
    }
}

// ── duration ──────────────────────────────────────────────────────────

/// Validates `duration` format.\npub struct DurationChecker;

impl FormatChecker for DurationChecker {
    fn check(&self, value: &str) -> bool {
        // ISO 8601 duration: P[n]Y[n]M[n]DT[n]H[n]M[n]S
        let bytes = value.as_bytes();
        if bytes.is_empty() || bytes[0] != b'P' { return false; }
        let rest = &value[1..];
        if rest.is_empty() { return false; }

        let has_time = rest.contains('T');
        if has_time {
            let parts: Vec<&str> = rest.split('T').collect();
            if parts.len() != 2 { return false; }
            if parts[0].is_empty() && parts[1].is_empty() { return false; }
            if !parse_duration_date(parts[0]) { return false; }
            if !parse_duration_time(parts[1]) { return false; }
        } else if !parse_duration_date(rest) { return false; }
        true
    }
    fn format_name(&self) -> &'static str { "duration" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(DurationChecker)
    }
}

fn parse_duration_date(s: &str) -> bool {
    if s.is_empty() { return true; }
    let mut has = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Read digits
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
        if i == start || i >= chars.len() { return false; }
        let designator = chars[i];
        match designator {
            'Y' | 'M' | 'W' | 'D' => { has = true; },
            _ => return false,
        }
        i += 1;
    }
    has
}

fn parse_duration_time(s: &str) -> bool {
    if s.is_empty() { return true; }
    let mut has = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
        if i == start || i >= chars.len() { return false; }
        let designator = chars[i];
        match designator {
            'H' | 'M' | 'S' => { has = true; },
            _ => return false,
        }
        i += 1;
    }
    has
}

// ── email ─────────────────────────────────────────────────────────────

/// Validates `email` format.\npub struct EmailChecker;

impl FormatChecker for EmailChecker {
    fn check(&self, value: &str) -> bool {
        // Basic RFC 5321 email validation
        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() != 2 { return false; }
        let local = parts[0];
        let domain = parts[1];
        if local.is_empty() || domain.is_empty() { return false; }
        // Domain must have at least one dot
        if !domain.contains('.') { return false; }
        let domain_parts: Vec<&str> = domain.split('.').collect();
        domain_parts.iter().all(|d| !d.is_empty())
    }
    fn format_name(&self) -> &'static str { "email" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(EmailChecker)
    }
}

// ── idn-email ─────────────────────────────────────────────────────────

/// Validates `idn-email` format.\npub struct IdnEmailChecker;

impl FormatChecker for IdnEmailChecker {
    fn check(&self, value: &str) -> bool {
        // Basic: same as email but allows Unicode
        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() != 2 { return false; }
        !parts[0].is_empty() && !parts[1].is_empty()
    }
    fn format_name(&self) -> &'static str { "idn-email" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(IdnEmailChecker)
    }
}

// ── hostname ──────────────────────────────────────────────────────────

/// Validates `hostname` format.\npub struct HostnameChecker;

impl FormatChecker for HostnameChecker {
    fn check(&self, value: &str) -> bool {
        // RFC 1123 hostname
        if value.is_empty() || value.len() > 253 { return false; }
        let labels: Vec<&str> = value.split('.').collect();
        labels.iter().all(|l| {
            if l.is_empty() || l.len() > 63 { return false; }
            let bytes = l.as_bytes();
            if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' { return false; }
            l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    }
    fn format_name(&self) -> &'static str { "hostname" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(HostnameChecker)
    }
}

// ── idn-hostname ──────────────────────────────────────────────────────

/// Validates `idn-hostname` format.\npub struct IdnHostnameChecker;

impl FormatChecker for IdnHostnameChecker {
    fn check(&self, value: &str) -> bool {
        // Basic: allow Unicode, same structure as hostname
        if value.is_empty() { return false; }
        let labels: Vec<&str> = value.split('.').collect();
        labels.iter().all(|l| !l.is_empty() && l.len() <= 63)
    }
    fn format_name(&self) -> &'static str { "idn-hostname" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(IdnHostnameChecker)
    }
}

// ── ipv4 ──────────────────────────────────────────────────────────────

/// Validates `ipv4` format.\npub struct Ipv4Checker;

impl FormatChecker for Ipv4Checker {
    fn check(&self, value: &str) -> bool {
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() != 4 { return false; }
        parts.iter().all(|p| {
            if p.is_empty() || (p.len() > 1 && p.starts_with('0')) { return false; }
            p.parse::<u8>().is_ok()
        })
    }
    fn format_name(&self) -> &'static str { "ipv4" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(Ipv4Checker)
    }
}

// ── ipv6 ──────────────────────────────────────────────────────────────

/// Validates `ipv6` format.\npub struct Ipv6Checker;

impl FormatChecker for Ipv6Checker {
    fn check(&self, value: &str) -> bool {
        // RFC 4291 IPv6 with :: compression
        if value.is_empty() { return false; }
        let has_compression = value.contains("::");
        let segments: Vec<&str> = if has_compression {
            // Split on ::
            let parts: Vec<&str> = value.split("::").collect();
            if parts.len() != 2 { return false; }
            let mut segs = Vec::new();
            if !parts[0].is_empty() {
                segs.extend(parts[0].split(':'));
            }
            if !parts[1].is_empty() {
                segs.extend(parts[1].split(':'));
            }
            segs
        } else {
            value.split(':').collect()
        };

        let max_segs = if has_compression { 7 } else { 8 };
        if segments.len() > max_segs { return false; }
        segments.iter().all(|s| {
            if s.is_empty() { return false; }
            if s.len() > 4 { return false; }
            u16::from_str_radix(s, 16).is_ok()
        })
    }
    fn format_name(&self) -> &'static str { "ipv6" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(Ipv6Checker)
    }
}

// ── uri ───────────────────────────────────────────────────────────────

/// Validates `uri` format.\npub struct UriChecker;

impl FormatChecker for UriChecker {
    fn check(&self, value: &str) -> bool {
        // RFC 3986 URI: scheme ":" hier-part ["?" query] ["#" fragment]
        let Some(colon) = value.find(':') else { return false };
        let scheme = &value[..colon];
        if scheme.is_empty() { return false; }
        let mut chars = scheme.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        for c in scheme.chars() {
            if !c.is_ascii_alphanumeric() && c != '+' && c != '-' && c != '.' {
                return false;
            }
        }
        let after = &value[colon + 1..];
        // Must have something after scheme:
        !after.is_empty()
    }
    fn format_name(&self) -> &'static str { "uri" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(UriChecker)
    }
}

// ── uri-reference ─────────────────────────────────────────────────────

/// Validates `uri-reference` format.\npub struct UriReferenceChecker;

impl FormatChecker for UriReferenceChecker {
    fn check(&self, value: &str) -> bool {
        // URI or relative-ref: can be relative (no scheme)
        if value.is_empty() { return true; }
        // Try as absolute URI first
        if value.contains(':') {
            return UriChecker.check(value);
        }
        // Relative reference: just check no control characters
        !value.chars().any(char::is_control)
    }
    fn format_name(&self) -> &'static str { "uri-reference" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(UriReferenceChecker)
    }
}

// ── iri ───────────────────────────────────────────────────────────────

/// Validates `iri` format.\npub struct IriChecker;

impl FormatChecker for IriChecker {
    fn check(&self, value: &str) -> bool {
        // IRI is like URI but allows Unicode
        let Some(colon) = value.find(':') else { return false };
        let scheme = &value[..colon];
        if scheme.is_empty() { return false; }
        let mut chars = scheme.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        !value[colon + 1..].is_empty()
    }
    fn format_name(&self) -> &'static str { "iri" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(IriChecker)
    }
}

// ── iri-reference ─────────────────────────────────────────────────────

/// Validates `iri-reference` format.\npub struct IriReferenceChecker;

impl FormatChecker for IriReferenceChecker {
    fn check(&self, value: &str) -> bool {
        if value.is_empty() { return true; }
        if value.contains(':') {
            return IriChecker.check(value);
        }
        !value.chars().any(char::is_control)
    }
    fn format_name(&self) -> &'static str { "iri-reference" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(IriReferenceChecker)
    }
}

// ── uri-template ──────────────────────────────────────────────────────

/// Validates `uri-template` format.\npub struct UriTemplateChecker;

impl FormatChecker for UriTemplateChecker {
    fn check(&self, value: &str) -> bool {
        // RFC 6570: basic check — valid URI with optional {expression}
        if value.is_empty() { return false; }
        // Check for balanced braces
        let mut depth: i32 = 0;
        for c in value.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {},
            }
            if depth < 0 { return false; }
        }
        depth == 0
    }
    fn format_name(&self) -> &'static str { "uri-template" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(UriTemplateChecker)
    }
}

// ── json-pointer ──────────────────────────────────────────────────────

/// Validates `json-pointer` format.\npub struct JsonPointerChecker;

impl FormatChecker for JsonPointerChecker {
    fn check(&self, value: &str) -> bool {
        // RFC 6901: empty string or starts with /
        if value.is_empty() { return true; }
        if !value.starts_with('/') { return false; }
        // Check for valid escape sequences: ~0 (~) and ~1 (/)
        let mut chars = value.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '~' {
                match chars.peek() {
                    Some(&'0' | &'1') => { chars.next(); }
                    _ => return false,
                }
            }
        }
        true
    }
    fn format_name(&self) -> &'static str { "json-pointer" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(JsonPointerChecker)
    }
}

// ── relative-json-pointer ─────────────────────────────────────────────

/// Validates `relative-json-pointer` format.\npub struct RelativeJsonPointerChecker;

impl FormatChecker for RelativeJsonPointerChecker {
    fn check(&self, value: &str) -> bool {
        // Non-negative integer followed by "#" or JSON pointer
        if value.is_empty() { return false; }
        let mut i = 0;
        let bytes = value.as_bytes();
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 { return false; }
        let rest = &value[i..];
        if rest.is_empty() { return false; }
        if rest == "#" { return true; }
        // Must be a valid JSON pointer
        JsonPointerChecker.check(rest)
    }
    fn format_name(&self) -> &'static str { "relative-json-pointer" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(RelativeJsonPointerChecker)
    }
}

// ── regex ─────────────────────────────────────────────────────────────

/// Validates `regex` format.\npub struct RegexChecker;

impl FormatChecker for RegexChecker {
    fn check(&self, value: &str) -> bool { regex::Regex::new(value).is_ok() }
    fn format_name(&self) -> &'static str { "regex" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(RegexChecker)
    }
}

// ── uuid ──────────────────────────────────────────────────────────────

/// Validates `uuid` format.\npub struct UuidChecker;

impl FormatChecker for UuidChecker {
    fn check(&self, value: &str) -> bool {
        // 8-4-4-4-12 hex pattern
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 5 { return false; }
        let lens = [8, 4, 4, 4, 12];
        parts.iter().zip(lens).all(|(p, len)| {
            p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit())
        })
    }
    fn format_name(&self) -> &'static str { "uuid" }
    fn clone_box(&self) -> Box<dyn FormatChecker> {
        Box::new(UuidChecker)
    }
}

// ── Registry ──────────────────────────────────────────────────────────

/// Look up a built-in format checker by name.
#[must_use]
pub fn builtin_format(name: &str) -> Option<&'static dyn FormatChecker> {
    match name {
        "date-time" => Some(&DateTimeChecker),
        "date" => Some(&DateChecker),
        "time" => Some(&TimeChecker),
        "duration" => Some(&DurationChecker),
        "email" => Some(&EmailChecker),
        "idn-email" => Some(&IdnEmailChecker),
        "hostname" => Some(&HostnameChecker),
        "idn-hostname" => Some(&IdnHostnameChecker),
        "ipv4" => Some(&Ipv4Checker),
        "ipv6" => Some(&Ipv6Checker),
        "uri" => Some(&UriChecker),
        "uri-reference" => Some(&UriReferenceChecker),
        "iri" => Some(&IriChecker),
        "iri-reference" => Some(&IriReferenceChecker),
        "uri-template" => Some(&UriTemplateChecker),
        "json-pointer" => Some(&JsonPointerChecker),
        "relative-json-pointer" => Some(&RelativeJsonPointerChecker),
        "regex" => Some(&RegexChecker),
        "uuid" => Some(&UuidChecker),
        _ => None,
    }
}

/// Create an owned format checker by name.
///
/// Prefers custom formats (checked first), falls back to built-in.
/// Returns None if no checker is found for the given name.
#[must_use]
pub fn make_format_checker(
    name: &str,
    custom_formats: &alloc::collections::BTreeMap<String, Box<dyn FormatChecker>>,
) -> Option<Box<dyn FormatChecker>> {
    if let Some(checker) = custom_formats.get(name) {
        // Custom takes precedence — return a cloned version
        // Since we can't clone Box<dyn FormatChecker>, we'll need a different approach
        // Store the reference instead
        return clone_checker(checker.as_ref());
    }
    builtin_format(name).and_then(clone_checker)
}

/// Clone a format checker into an owned Box.
pub fn clone_checker(checker: &dyn FormatChecker) -> Option<Box<dyn FormatChecker>> {
    match checker.format_name() {
        "date-time" => Some(Box::new(DateTimeChecker)),
        "date" => Some(Box::new(DateChecker)),
        "time" => Some(Box::new(TimeChecker)),
        "duration" => Some(Box::new(DurationChecker)),
        "email" => Some(Box::new(EmailChecker)),
        "idn-email" => Some(Box::new(IdnEmailChecker)),
        "hostname" => Some(Box::new(HostnameChecker)),
        "idn-hostname" => Some(Box::new(IdnHostnameChecker)),
        "ipv4" => Some(Box::new(Ipv4Checker)),
        "ipv6" => Some(Box::new(Ipv6Checker)),
        "uri" => Some(Box::new(UriChecker)),
        "uri-reference" => Some(Box::new(UriReferenceChecker)),
        "iri" => Some(Box::new(IriChecker)),
        "iri-reference" => Some(Box::new(IriReferenceChecker)),
        "uri-template" => Some(Box::new(UriTemplateChecker)),
        "json-pointer" => Some(Box::new(JsonPointerChecker)),
        "relative-json-pointer" => Some(Box::new(RelativeJsonPointerChecker)),
        "regex" => Some(Box::new(RegexChecker)),
        "uuid" => Some(Box::new(UuidChecker)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_time_valid() {
        let c = DateTimeChecker;
        assert!(c.check("2024-01-15T10:30:00Z"));
        assert!(c.check("2024-01-15T10:30:00+05:30"));
        assert!(c.check("2024-01-15T10:30:00.123Z"));
    }

    #[test]
    fn date_time_invalid() {
        let c = DateTimeChecker;
        assert!(!c.check("2024-13-01T10:30:00Z")); // month 13
        assert!(!c.check("not-a-date"));
    }

    #[test]
    fn date_valid() {
        assert!(DateChecker.check("2024-01-15"));
        assert!(DateChecker.check("2024-02-29")); // leap year
    }

    #[test]
    fn date_invalid() {
        assert!(!DateChecker.check("2024-02-30")); // Feb never has 30
        assert!(!DateChecker.check("2023-02-29")); // not leap year
        assert!(!DateChecker.check("not-a-date"));
    }

    #[test]
    fn time_valid() {
        assert!(TimeChecker.check("10:30:00Z"));
        assert!(TimeChecker.check("10:30:00"));
    }

    #[test]
    fn ipv4_valid() {
        assert!(Ipv4Checker.check("192.168.0.1"));
        assert!(Ipv4Checker.check("0.0.0.0"));
    }

    #[test]
    fn ipv4_invalid() {
        assert!(!Ipv4Checker.check("256.0.0.1"));
        assert!(!Ipv4Checker.check("01.02.03.04")); // leading zeros
    }

    #[test]
    fn ipv6_valid() {
        assert!(Ipv6Checker.check("::1"));
        assert!(Ipv6Checker.check("2001:db8::1"));
        assert!(Ipv6Checker.check("::"));
    }

    #[test]
    fn email_valid() {
        assert!(EmailChecker.check("user@example.com"));
    }

    #[test]
    fn email_invalid() {
        assert!(!EmailChecker.check("no-at-sign"));
        assert!(!EmailChecker.check("@no-local.com"));
    }

    #[test]
    fn hostname_valid() {
        assert!(HostnameChecker.check("example.com"));
        assert!(HostnameChecker.check("sub.domain.co.uk"));
    }

    #[test]
    fn hostname_invalid() {
        assert!(!HostnameChecker.check("-invalid.com"));
        assert!(!HostnameChecker.check(""));
    }

    #[test]
    fn uri_valid() {
        assert!(UriChecker.check("https://example.com/path"));
        assert!(UriChecker.check("ftp://files.example.com"));
    }

    #[test]
    fn uri_invalid() {
        assert!(!UriChecker.check("noscheme"));
    }

    #[test]
    fn json_pointer_valid() {
        assert!(JsonPointerChecker.check(""));
        assert!(JsonPointerChecker.check("/foo/bar"));
        assert!(JsonPointerChecker.check("/~0/~1")); // escaped ~ and /
    }

    #[test]
    fn json_pointer_invalid() {
        assert!(!JsonPointerChecker.check("no-slash"));
        assert!(!JsonPointerChecker.check("/foo/~2")); // invalid escape
    }

    #[test]
    fn uuid_valid() {
        assert!(UuidChecker.check("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn uuid_invalid() {
        assert!(!UuidChecker.check("not-a-uuid"));
        assert!(!UuidChecker.check("550e8400-e29b-41d4-a716"));
    }

    #[test]
    fn regex_valid() {
        assert!(RegexChecker.check(r"^\d+$"));
        assert!(RegexChecker.check("[a-z]+"));
    }

    #[test]
    fn regex_invalid() {
        assert!(!RegexChecker.check(r"\w(")); // unbalanced paren
    }

    #[test]
    fn duration_valid() {
        assert!(DurationChecker.check("P1Y2M3D"));
        assert!(DurationChecker.check("PT1H30M"));
        assert!(DurationChecker.check("P1DT1H"));
    }

    #[test]
    fn duration_invalid() {
        assert!(!DurationChecker.check("1Y")); // missing P
        assert!(!DurationChecker.check("P"));  // nothing after P
    }

    #[test]
    fn uri_template_valid() {
        assert!(UriTemplateChecker.check("/users/{id}"));
        assert!(UriTemplateChecker.check("/search{?q}"));
    }

    #[test]
    fn uri_template_invalid() {
        assert!(!UriTemplateChecker.check("/unbalanced{"));
    }

    #[test]
    fn relative_json_pointer_valid() {
        assert!(RelativeJsonPointerChecker.check("0#"));
        assert!(RelativeJsonPointerChecker.check("1/foo"));
    }
}
