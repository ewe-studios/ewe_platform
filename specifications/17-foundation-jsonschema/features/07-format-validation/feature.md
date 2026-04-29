---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
feature_directory: "specifications/17-foundation-jsonschema/features/07-format-validation"
this_file: "specifications/17-foundation-jsonschema/features/07-format-validation/feature.md"

status: complete
priority: medium
created: 2026-04-28

depends_on: ["00-core-types", "02-keywords-validators"]

tasks:
  completed: 22
  uncompleted: 0
  total: 22
  completion_percentage: 100
---

# Feature 7: Format Validation

## Overview

Implement built-in format validators for the `format` keyword. JSON Schema defines several standard format names (e.g., `email`, `uri`, `date-time`, `ipv4`, `uuid`) with specific validation rules. The `format` keyword is annotation-only by default (does not cause validation failure) but can be made assertive via `ValidationOptions::assert_format(true)`.

Derived from `crates/jsonschema/src/keywords/format.rs` and associated format-specific code in the reference project.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | All implementation | `.agents/skills/rust-clean-code/skill.md` |

### Pre-Implementation Checklist

- [ ] Read `.agents/skills/rust-clean-code/skill.md`
- [ ] Read Feature 0 (core-types) and Feature 2 (keywords)

---

## Requirements

1. **FormatChecker trait** — A trait for checking whether a string matches a format:
   ```rust
   pub trait FormatChecker: Send + Sync {
       fn check(&self, value: &str) -> bool;
       fn format_name(&self) -> &str;
   }
   ```

2. **Built-in formats** — Implement checkers for all standard formats:
   - `date-time` — RFC 3339 date-time (e.g., `"2024-01-15T10:30:00Z"`)
   - `date` — RFC 3339 full-date (e.g., `"2024-01-15"`)
   - `time` — RFC 3339 full-time (e.g., `"10:30:00Z"`)
   - `duration` — ISO 8601 duration (e.g., `"P1Y2M3D"`)
   - `email` — RFC 5321 email address
   - `idn-email` — RFC 6531 internationalized email (basic support)
   - `hostname` — RFC 1123 hostname
   - `idn-hostname` — RFC 5890 internationalized hostname (basic support)
   - `ipv4` — RFC 2673 IPv4 address
   - `ipv6` — RFC 4291 IPv6 address
   - `uri` — RFC 3986 URI
   - `uri-reference` — RFC 3986 URI Reference
   - `iri` — RFC 3987 IRI (basic support)
   - `iri-reference` — RFC 3987 IRI Reference (basic support)
   - `uri-template` — RFC 6570 URI Template (basic check)
   - `json-pointer` — RFC 6901 JSON Pointer
   - `relative-json-pointer` — Relative JSON Pointer
   - `regex` — ECMA-262 regex (validate that the string is a valid regex)
   - `uuid` — RFC 4122 UUID

3. **Assertion behavior** — By default, `format` is annotation-only (always valid). When `assert_format` is true in `ValidationOptions`, format validation failures produce errors.

4. **No external dependencies** — All format validation must be implemented with manual parsing or the `regex` crate already in the workspace. No `email_address`, `uuid-simd`, or `chrono` crates.

## Architecture (COMPREHENSIVE)

### Component Structure

```
backends/foundation_jsonschema/src/
├── keywords/
│   └── format.rs           # FormatValidator, FormatChecker trait, dispatch
└── formats/
    ├── mod.rs              # Format registry, lookup by name
    ├── datetime.rs         # date-time, date, time, duration
    ├── email.rs            # email, idn-email
    ├── hostname.rs         # hostname, idn-hostname
    ├── ip.rs               # ipv4, ipv6
    ├── uri_format.rs       # uri, uri-reference, iri, iri-reference, uri-template
    ├── pointer.rs          # json-pointer, relative-json-pointer
    ├── regex_format.rs     # regex (validate that string is valid regex)
    └── uuid.rs             # uuid
```

### Format Checker Implementations

Each format checker is a simple struct implementing `FormatChecker`. Example:

```rust
/// Validates RFC 2673 IPv4 addresses.
///
/// WHY: The "ipv4" format in JSON Schema requires dotted-decimal notation
/// with exactly 4 octets, each 0-255. No leading zeros (except "0" itself).
pub struct Ipv4Checker;

impl FormatChecker for Ipv4Checker {
    fn check(&self, value: &str) -> bool {
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() != 4 { return false; }
        parts.iter().all(|p| {
            if p.is_empty() || (p.len() > 1 && p.starts_with('0')) { return false; }
            p.parse::<u8>().is_ok()
        })
    }
    fn format_name(&self) -> &str { "ipv4" }
}
```

### Date-Time Parsing (no chrono dependency)

Implement manual RFC 3339 parsing:

```rust
/// Validates RFC 3339 date-time strings.
///
/// Format: YYYY-MM-DDThh:mm:ss[.fractional]Z or ±hh:mm
/// Examples: "2024-01-15T10:30:00Z", "2024-01-15T10:30:00+05:30"
pub struct DateTimeChecker;

impl FormatChecker for DateTimeChecker {
    fn check(&self, value: &str) -> bool {
        // Split on 'T' or 't'
        // Validate date part: YYYY-MM-DD with valid ranges
        // Validate time part: hh:mm:ss with valid ranges
        // Validate timezone: Z or ±hh:mm
        // Validate calendar correctness (leap years, month lengths)
        ...
    }
}
```

### Format Registry

```rust
/// Look up a built-in format checker by name.
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
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| Manual parsing (no chrono/email crates) | Zero external deps for formats, matches no-network-deps goal | chrono for dates (adds dependency tree); email_address crate (another dep) |
| Static dispatch via match | Fast, no allocation, compile-time verified | HashMap<String, Box<dyn FormatChecker>> (heap allocation, runtime cost) |
| Annotation-only by default | Matches JSON Schema spec — format is annotation, not assertion | Assert by default (breaks spec compliance) |

## Tasks

- [ ] Task 1: Define `FormatChecker` trait in `keywords/format.rs`
- [ ] Task 2: Implement `FormatValidator` that dispatches to format checkers (annotation vs assertion mode)
- [ ] Task 3: Create `formats/mod.rs` with `builtin_format()` registry function
- [ ] Task 4: Implement `DateTimeChecker` — RFC 3339 date-time with timezone
- [ ] Task 5: Implement `DateChecker` — RFC 3339 full-date (YYYY-MM-DD with calendar validation)
- [ ] Task 6: Implement `TimeChecker` — RFC 3339 full-time (hh:mm:ss with timezone)
- [ ] Task 7: Implement `DurationChecker` — ISO 8601 duration (P[n]Y[n]M[n]DT[n]H[n]M[n]S)
- [ ] Task 8: Implement `EmailChecker` — RFC 5321 basic email validation
- [ ] Task 9: Implement `IdnEmailChecker` — basic internationalized email
- [ ] Task 10: Implement `HostnameChecker` — RFC 1123 hostname (labels, length limits)
- [ ] Task 11: Implement `IdnHostnameChecker` — basic internationalized hostname
- [ ] Task 12: Implement `Ipv4Checker` — dotted-decimal, 4 octets, 0-255
- [ ] Task 13: Implement `Ipv6Checker` — full RFC 4291 with `::` compression, embedded IPv4
- [ ] Task 14: Implement `UriChecker` and `UriReferenceChecker` — reuse URI parser from referencing
- [ ] Task 15: Implement `IriChecker` and `IriReferenceChecker` — basic IRI validation
- [ ] Task 16: Implement `UriTemplateChecker` — basic RFC 6570 structure check
- [ ] Task 17: Implement `JsonPointerChecker` — RFC 6901 pointer format
- [ ] Task 18: Implement `RelativeJsonPointerChecker` — non-negative integer prefix + pointer
- [ ] Task 19: Implement `RegexChecker` — validate regex compilation
- [ ] Task 20: Implement `UuidChecker` — 8-4-4-4-12 hex pattern
- [ ] Task 21: Write unit tests for every format checker with valid and invalid examples
- [ ] Task 22: Write tests for assertion mode vs annotation mode behavior

## Success Criteria

- [ ] All tasks completed
- [ ] All tests passing
- [ ] All 19 standard format names implemented
- [ ] No external dependencies beyond regex (workspace)
- [ ] Annotation-only by default, assertive when configured
- [ ] Zero clippy warnings

---

_Created: 2026-04-28_
