//! The user of a command (feature 12, section 6.7).
//!
//! A command may name its user as `"uid"`, `"uid:gid"`, `"name"` or
//! `"name:group"`. A numeric first part is a user identifier (UID); any other
//! first part is a user name in the guest's `/etc/passwd`. A numeric second
//! part is a group identifier (GID); any other second part is a group name in
//! the guest's `/etc/group`.
//!
//! The slave reads the two guest files and calls [`resolve_user`]. The parser
//! is pure, so a test covers every form without a guest.

/// One resolved user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedUser {
    /// The user identifier.
    pub uid: u32,
    /// The group identifier.
    pub gid: u32,
    /// The home directory from `/etc/passwd`, or `/`.
    pub home: String,
}

/// Why one user specification is not usable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserError {
    /// The text is not one of the four forms.
    BadFormat {
        /// The text.
        text: String,
    },
    /// The guest `/etc/passwd` has no such user.
    UnknownUser {
        /// The user name.
        name: String,
    },
    /// The guest `/etc/group` has no such group.
    UnknownGroup {
        /// The group name.
        name: String,
    },
}

impl std::fmt::Display for UserError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadFormat { text } => write!(
                formatter,
                "the user must be `uid`, `uid:gid`, `name` or `name:group`: {text}"
            ),
            Self::UnknownUser { name } => write!(formatter, "the image has no user {name}"),
            Self::UnknownGroup { name } => write!(formatter, "the image has no group {name}"),
        }
    }
}

impl std::error::Error for UserError {}

/// One line of `/etc/passwd`.
struct PasswdEntry<'a> {
    name: &'a str,
    uid: u32,
    gid: u32,
    home: &'a str,
}

/// Resolves one user specification against the guest files.
///
/// # Errors
/// [`UserError`] for a bad form, an unknown user or an unknown group.
pub fn resolve_user(spec: &str, passwd: &str, group: &str) -> Result<ResolvedUser, UserError> {
    let text = spec.trim();
    if text.is_empty() {
        return Err(UserError::BadFormat {
            text: spec.to_string(),
        });
    }
    let (first, second) = match text.split_once(':') {
        Some((first, second)) => {
            if second.is_empty() {
                return Err(UserError::BadFormat {
                    text: spec.to_string(),
                });
            }
            (first, Some(second))
        }
        None => (text, None),
    };

    // The user part: a number is a UID, any other text is a name.
    let (uid, entry) = if let Ok(uid) = first.parse::<u32>() {
        (uid, find_uid(passwd, uid))
    } else {
        let entry = find_name(passwd, first).ok_or_else(|| UserError::UnknownUser {
            name: first.to_string(),
        })?;
        (entry.uid, Some(entry))
    };

    let gid = if let Some(second) = second {
        if let Ok(gid) = second.parse::<u32>() {
            gid
        } else {
            find_group(group, second).ok_or_else(|| UserError::UnknownGroup {
                name: second.to_string(),
            })?
        }
    } else {
        entry.as_ref().map_or(uid, |entry| entry.gid)
    };
    let home = entry
        .as_ref()
        .map_or_else(|| "/".to_string(), |entry| entry.home.to_string());
    Ok(ResolvedUser { uid, gid, home })
}

/// The `/etc/passwd` entry with this name.
fn find_name<'a>(passwd: &'a str, name: &str) -> Option<PasswdEntry<'a>> {
    passwd.lines().find_map(|line| {
        let entry = parse_passwd(line)?;
        (entry.name == name).then_some(entry)
    })
}

/// The `/etc/passwd` entry with this UID.
fn find_uid(passwd: &str, uid: u32) -> Option<PasswdEntry<'_>> {
    passwd.lines().find_map(|line| {
        let entry = parse_passwd(line)?;
        (entry.uid == uid).then_some(entry)
    })
}

/// One `/etc/passwd` line, or `None` for a comment or a short line.
fn parse_passwd(line: &str) -> Option<PasswdEntry<'_>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut fields = line.split(':');
    let name = fields.next()?;
    let _password = fields.next()?;
    let uid = fields.next()?.parse().ok()?;
    let gid = fields.next()?.parse().ok()?;
    let _gecos = fields.next()?;
    let home = fields.next()?;
    let _shell = fields.next()?;
    Some(PasswdEntry {
        name,
        uid,
        gid,
        home,
    })
}

/// The GID of the group with this name in `/etc/group`.
fn find_group(group: &str, name: &str) -> Option<u32> {
    group.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let mut fields = line.split(':');
        let group_name = fields.next()?;
        let _password = fields.next()?;
        let gid = fields.next()?.parse().ok()?;
        (group_name == name).then_some(gid)
    })
}
