//! URI authority component (userinfo@host:port).

use super::error::InvalidUri;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

/// URI authority component.
///
/// WHY: The authority identifies the server and optional authentication
/// credentials for a URI. It's critical for connection establishment.
///
/// WHAT: Represents `[userinfo@]host[:port]` with proper validation for
/// each component.
///
/// HOW: Parses and validates IPv4, IPv6, and domain name hosts along with
/// optional port and userinfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authority {
    /// Optional userinfo (user:pass)
    userinfo: Option<String>,
    /// Host component (required)
    host: Host,
    /// Optional port number
    port: Option<u16>,
}

/// Host component of authority.
///
/// Supports IPv4, IPv6, and domain names (reg-name in RFC 3986).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Host {
    /// IPv4 address
    Ipv4(Ipv4Addr),
    /// IPv6 address (in brackets)
    Ipv6(Ipv6Addr),
    /// Domain name or registered name
    RegName(String),
}

impl Authority {
    /// Parses authority and returns (Option<Authority>, remainder).
    ///
    /// # Arguments
    ///
    /// * `s` - String starting after "//" in URI
    ///
    /// # Returns
    ///
    /// Tuple of (Option<Authority>, remainder after authority)
    ///
    /// # Errors
    ///
    /// Returns `InvalidUri` if authority is malformed.
    pub(crate) fn parse_with_remainder(s: &str) -> Result<(Option<Self>, &str), InvalidUri> {
        if s.is_empty() {
            return Ok((None, s));
        }

        // Find the end of authority (first '/', '?', or '#')
        let authority_end = s.find(&['/', '?', '#'][..]).unwrap_or(s.len());

        let authority_str = &s[..authority_end];
        let remainder = &s[authority_end..];

        if authority_str.is_empty() {
            return Ok((None, remainder));
        }

        let authority = Self::parse(authority_str)?;
        Ok((Some(authority), remainder))
    }

    /// Parses just the authority component.
    fn parse(s: &str) -> Result<Self, InvalidUri> {
        // Split off userinfo if present (before last @)
        let (userinfo, host_port) = if let Some(at_pos) = s.rfind('@') {
            let userinfo = &s[..at_pos];
            let userinfo = if userinfo.is_empty() {
                None
            } else {
                Some(userinfo.to_string())
            };
            (userinfo, &s[at_pos + 1..])
        } else {
            (None, s)
        };

        // Parse host and port
        let (host, port) = Self::parse_host_port(host_port)?;

        Ok(Authority {
            userinfo,
            host,
            port,
        })
    }

    /// Parses host:port component.
    fn parse_host_port(s: &str) -> Result<(Host, Option<u16>), InvalidUri> {
        // Check for IPv6 (enclosed in brackets)
        if s.starts_with('[') {
            let close_bracket = s
                .find(']')
                .ok_or_else(|| InvalidUri::new("unclosed IPv6 bracket"))?;

            let ipv6_str = &s[1..close_bracket];
            let ipv6: Ipv6Addr = ipv6_str
                .parse()
                .map_err(|_| InvalidUri::new(format!("invalid IPv6 address: {}", ipv6_str)))?;

            // Check for port after bracket
            let after_bracket = &s[close_bracket + 1..];
            let port = if after_bracket.starts_with(':') {
                Some(Self::parse_port(&after_bracket[1..])?)
            } else if after_bracket.is_empty() {
                None
            } else {
                return Err(InvalidUri::new("invalid characters after IPv6 bracket"));
            };

            return Ok((Host::Ipv6(ipv6), port));
        }

        // Not IPv6, split on last colon for port
        if let Some(colon_pos) = s.rfind(':') {
            let host_part = &s[..colon_pos];
            let port_part = &s[colon_pos + 1..];

            // Try parsing as IPv4 first
            if let Ok(ipv4) = host_part.parse::<Ipv4Addr>() {
                let port = Some(Self::parse_port(port_part)?);
                return Ok((Host::Ipv4(ipv4), port));
            }

            // Otherwise it's a domain name with port
            if host_part.is_empty() {
                return Err(InvalidUri::new("empty host"));
            }

            Self::validate_reg_name(host_part)?;
            let port = Some(Self::parse_port(port_part)?);
            Ok((Host::RegName(host_part.to_string()), port))
        } else {
            // No colon, just host
            if s.is_empty() {
                return Err(InvalidUri::new("empty host"));
            }

            // Try IPv4
            if let Ok(ipv4) = s.parse::<Ipv4Addr>() {
                return Ok((Host::Ipv4(ipv4), None));
            }

            // Otherwise reg-name
            Self::validate_reg_name(s)?;
            Ok((Host::RegName(s.to_string()), None))
        }
    }

    /// Parses a port string.
    fn parse_port(s: &str) -> Result<u16, InvalidUri> {
        s.parse::<u16>()
            .map_err(|_| InvalidUri::new(format!("invalid port: {}", s)))
    }

    /// Validates a reg-name (domain name) according to RFC 3986.
    ///
    /// Allows: unreserved / pct-encoded / sub-delims
    /// unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~"
    /// sub-delims = "!" / "$" / "&" / "'" / "(" / ")" / "*" / "+" / "," / ";" / "="
    fn validate_reg_name(s: &str) -> Result<(), InvalidUri> {
        for c in s.chars() {
            if !(c.is_ascii_alphanumeric()
                || c == '-'
                || c == '.'
                || c == '_'
                || c == '~'
                || c == '!'
                || c == '$'
                || c == '&'
                || c == '\''
                || c == '('
                || c == ')'
                || c == '*'
                || c == '+'
                || c == ','
                || c == ';'
                || c == '='
                || c == '%')
            // pct-encoded
            {
                return Err(InvalidUri::new(format!("invalid host character: {}", c)));
            }
        }
        Ok(())
    }

    /// Returns the host component.
    #[must_use]
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Returns the port if present.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// Returns the userinfo if present.
    #[must_use]
    pub fn userinfo(&self) -> Option<&str> {
        self.userinfo.as_deref()
    }
}

impl Host {
    /// Returns the host as a string slice.
    ///
    /// WHY: We need a way to get the host as a &str for display and comparison.
    ///
    /// WHAT: For domain names, returns the string directly. For IP addresses,
    /// this method cannot return &str without allocation since we need to format
    /// the IP address. Use `to_string_for_display()` for IP addresses.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Note
    ///
    /// For IP addresses (IPv4/IPv6), use `to_string_for_display()` instead,
    /// or pattern match on the enum to access the underlying `Ipv4Addr`/`Ipv6Addr`.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Host::Ipv4(_) => None,
            Host::Ipv6(_) => None,
            Host::RegName(name) => Some(name),
        }
    }

    /// Returns the host as an IPv4 address if it is one.
    #[must_use]
    pub fn as_ipv4(&self) -> Option<Ipv4Addr> {
        match self {
            Host::Ipv4(addr) => Some(*addr),
            _ => None,
        }
    }

    /// Returns the host as an IPv6 address if it is one.
    #[must_use]
    pub fn as_ipv6(&self) -> Option<Ipv6Addr> {
        match self {
            Host::Ipv6(addr) => Some(*addr),
            _ => None,
        }
    }

    /// Returns true if this is an IP address (IPv4 or IPv6).
    #[must_use]
    pub fn is_ip_addr(&self) -> bool {
        matches!(self, Host::Ipv4(_) | Host::Ipv6(_))
    }

    /// Returns the host as a string for display purposes.
    ///
    /// WHY: Display representation needs to handle IPv6 brackets and format
    /// IP addresses properly.
    ///
    /// WHAT: Returns a formatted string representation suitable for URIs:
    /// - IPv4: "192.168.1.1"
    /// - IPv6: "[::1]" (with brackets for URI)
    /// - RegName: "example.com"
    ///
    /// HOW: Uses Rust's built-in Display implementations for IP addresses.
    pub(crate) fn to_string_for_display(&self) -> String {
        match self {
            Host::Ipv4(addr) => addr.to_string(),
            Host::Ipv6(addr) => format!("[{}]", addr),
            Host::RegName(name) => name.clone(),
        }
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(userinfo) = &self.userinfo {
            write!(f, "{}@", userinfo)?;
        }

        write!(f, "{}", self.host.to_string_for_display())?;

        if let Some(port) = self.port {
            write!(f, ":{}", port)?;
        }

        Ok(())
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_for_display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authority_parse_domain() {
        let auth = Authority::parse("example.com").unwrap();
        assert!(matches!(auth.host(), Host::RegName(_)));
        assert_eq!(auth.port(), None);
        assert_eq!(auth.userinfo(), None);
    }

    #[test]
    fn test_authority_parse_domain_with_port() {
        let auth = Authority::parse("example.com:8080").unwrap();
        assert_eq!(auth.port(), Some(8080));
    }

    #[test]
    fn test_authority_parse_ipv4() {
        let auth = Authority::parse("192.168.1.1").unwrap();
        assert!(matches!(auth.host(), Host::Ipv4(_)));
        assert_eq!(auth.port(), None);
    }

    #[test]
    fn test_authority_parse_ipv4_with_port() {
        let auth = Authority::parse("192.168.1.1:8080").unwrap();
        assert!(matches!(auth.host(), Host::Ipv4(_)));
        assert_eq!(auth.port(), Some(8080));
    }

    #[test]
    fn test_authority_parse_ipv6() {
        let auth = Authority::parse("[::1]").unwrap();
        assert!(matches!(auth.host(), Host::Ipv6(_)));
        assert_eq!(auth.port(), None);
    }

    #[test]
    fn test_authority_parse_ipv6_with_port() {
        let auth = Authority::parse("[2001:db8::1]:8080").unwrap();
        assert!(matches!(auth.host(), Host::Ipv6(_)));
        assert_eq!(auth.port(), Some(8080));
    }

    #[test]
    fn test_authority_parse_with_userinfo() {
        let auth = Authority::parse("user:pass@example.com:8080").unwrap();
        assert_eq!(auth.userinfo(), Some("user:pass"));
        assert_eq!(auth.port(), Some(8080));
    }

    #[test]
    fn test_authority_parse_invalid_port() {
        assert!(Authority::parse("example.com:99999").is_err());
        assert!(Authority::parse("example.com:abc").is_err());
    }

    #[test]
    fn test_authority_parse_invalid_ipv6() {
        assert!(Authority::parse("[not::valid").is_err()); // Unclosed bracket
        assert!(Authority::parse("[zzz::1]").is_err()); // Invalid IPv6
    }

    #[test]
    fn test_authority_display() {
        let auth = Authority::parse("user@example.com:8080").unwrap();
        assert_eq!(auth.to_string(), "user@example.com:8080");

        let auth = Authority::parse("[::1]:443").unwrap();
        assert_eq!(auth.to_string(), "[::1]:443");
    }
}
