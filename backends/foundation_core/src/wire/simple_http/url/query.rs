//! Query string parsing and manipulation.
//!
//! WHY: Query strings contain key-value parameters that need parsing,
//! encoding, and manipulation for HTTP operations.
//!
//! WHAT: Provides utilities for parsing query strings into key-value pairs
//! and building query strings from components.
//!
//! HOW: Handles percent-encoding/decoding and multiple values per key.

use std::fmt;

/// Query string parser and builder.
///
/// WHY: HTTP query strings need to be parsed into structured key-value pairs
/// for parameter extraction and URL manipulation.
///
/// WHAT: Represents a parsed query string as a vector of (key, value) tuples.
/// Supports percent-decoding and encoding.
///
/// HOW: Parses query strings by splitting on '&' and '=', handling percent-encoding.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::url::Query;
///
/// // Parse a query string
/// let query = Query::parse("key=value&foo=bar").unwrap();
/// assert_eq!(query.get("key"), Some("value"));
/// assert_eq!(query.get("foo"), Some("bar"));
///
/// // Build a query string
/// let mut query = Query::new();
/// query.append("search", "rust programming");
/// query.append("page", "1");
/// assert_eq!(query.to_string(), "search=rust+programming&page=1");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Query {
    /// Key-value pairs in order of appearance
    pairs: Vec<(String, String)>,
}

impl Query {
    /// Creates a new empty Query.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Query;
    ///
    /// let query = Query::new();
    /// assert!(query.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    /// Parses a query string into key-value pairs.
    ///
    /// # Purpose (WHY)
    ///
    /// Query strings from URLs need to be parsed into structured data for
    /// easy access to individual parameters.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `s` - Query string (without leading '?')
    ///
    /// # Returns (HOW)
    ///
    /// Parsed Query with decoded key-value pairs.
    ///
    /// # Errors
    ///
    /// Returns error if percent-encoding is malformed.
    ///
    /// # Panics
    ///
    /// This function does not panic. Malformed encoding returns `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Query;
    ///
    /// let query = Query::parse("key=value&foo=bar").unwrap();
    /// assert_eq!(query.get("key"), Some("value"));
    /// ```
    pub fn parse(s: &str) -> Result<Self, QueryError> {
        if s.is_empty() {
            return Ok(Self::new());
        }

        let mut pairs = Vec::new();

        for pair_str in s.split('&') {
            if pair_str.is_empty() {
                continue;
            }

            // Split on first '=' to handle values with '=' in them
            if let Some(eq_pos) = pair_str.find('=') {
                let key = &pair_str[..eq_pos];
                let value = &pair_str[eq_pos + 1..];

                let decoded_key = percent_decode(key)?;
                let decoded_value = percent_decode(value)?;

                pairs.push((decoded_key, decoded_value));
            } else {
                // Key without value
                let decoded_key = percent_decode(pair_str)?;
                pairs.push((decoded_key, String::new()));
            }
        }

        Ok(Self { pairs })
    }

    /// Appends a key-value pair to the query.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Query;
    ///
    /// let mut query = Query::new();
    /// query.append("key", "value");
    /// assert_eq!(query.get("key"), Some("value"));
    /// ```
    pub fn append(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.pairs.push((key.into(), value.into()));
    }

    /// Gets the first value for a given key.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Query;
    ///
    /// let query = Query::parse("key=value&key=other").unwrap();
    /// assert_eq!(query.get("key"), Some("value"));
    /// ```
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Gets all values for a given key.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Query;
    ///
    /// let query = Query::parse("key=value1&key=value2").unwrap();
    /// let values: Vec<&str> = query.get_all("key");
    /// assert_eq!(values, vec!["value1", "value2"]);
    /// ```
    #[must_use]
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        self.pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Returns true if the query is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Returns the number of key-value pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Returns an iterator over key-value pairs.
    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (key, value)) in self.pairs.iter().enumerate() {
            if i > 0 {
                write!(f, "&")?;
            }
            write!(f, "{}={}", percent_encode(key), percent_encode(value))?;
        }
        Ok(())
    }
}

/// Error returned when query parsing fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    message: String,
}

impl QueryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "query parse error: {}", self.message)
    }
}

impl std::error::Error for QueryError {}

/// Percent-decodes a string (application/x-www-form-urlencoded).
///
/// WHY: Query string values are percent-encoded and need decoding.
///
/// WHAT: Converts '+' to space and decodes %XX sequences.
///
/// HOW: Iterates through characters, handling '+' and %XX patterns.
///
/// # Panics
///
/// This function does not panic. Invalid encoding returns `Err(QueryError)`.
pub fn percent_decode(s: &str) -> Result<String, QueryError> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        match c {
            '+' => result.push(' '),
            '%' => {
                // Need exactly 2 hex digits
                let hex1 = chars.next().ok_or_else(|| {
                    QueryError::new("incomplete percent-encoding (missing first hex digit)")
                })?;
                let hex2 = chars.next().ok_or_else(|| {
                    QueryError::new("incomplete percent-encoding (missing second hex digit)")
                })?;

                let hex_str = format!("{hex1}{hex2}");
                let byte = u8::from_str_radix(&hex_str, 16).map_err(|_| {
                    QueryError::new(format!("invalid percent-encoding: %{hex_str}"))
                })?;

                result.push(byte as char);
            }
            _ => result.push(c),
        }
    }

    Ok(result)
}

/// Percent-encodes a string (application/x-www-form-urlencoded).
///
/// WHY: Query string values must be percent-encoded for safe URL transmission.
///
/// WHAT: Converts spaces to '+' and encodes special characters as %XX.
///
/// HOW: Encodes all characters except unreserved (alphanumeric, -, _, ., ~).
///
/// # Panics
///
/// This function does not panic.
#[must_use] 
pub fn percent_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);

    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push('%');
                result.push_str(&format!("{byte:02X}"));
            }
        }
    }

    result
}
