//! The user rows of section 7 (feature 12): each form of a command user and the
//! refusal of an unknown name. The parser is pure, so no guest runs.

use foundation_sandbox::shared::user::{resolve_user, ResolvedUser, UserError};

const PASSWD: &str = "\
root:x:0:0:root:/root:/bin/sh
alice:x:1000:1000:Alice:/home/alice:/bin/sh
";
const GROUP: &str = "\
root:x:0:
wheel:x:10:alice
staff:x:50:
";

fn resolve(spec: &str) -> Result<ResolvedUser, UserError> {
    resolve_user(spec, PASSWD, GROUP)
}

#[test]
fn user_parse() {
    assert_eq!(
        resolve("0"),
        Ok(ResolvedUser {
            uid: 0,
            gid: 0,
            home: "/root".to_string(),
        })
    );
    assert_eq!(
        resolve("1000:1000"),
        Ok(ResolvedUser {
            uid: 1000,
            gid: 1000,
            home: "/home/alice".to_string(),
        })
    );
    assert_eq!(
        resolve("alice"),
        Ok(ResolvedUser {
            uid: 1000,
            gid: 1000,
            home: "/home/alice".to_string(),
        })
    );
    assert_eq!(
        resolve("alice:wheel"),
        Ok(ResolvedUser {
            uid: 1000,
            gid: 10,
            home: "/home/alice".to_string(),
        })
    );
    assert_eq!(
        resolve("1000:50"),
        Ok(ResolvedUser {
            uid: 1000,
            gid: 50,
            home: "/home/alice".to_string(),
        })
    );
    // A user that the files do not name keeps the UID as the GID and `/` as the
    // home directory.
    assert_eq!(
        resolve("4242"),
        Ok(ResolvedUser {
            uid: 4242,
            gid: 4242,
            home: "/".to_string(),
        })
    );
}

#[test]
fn user_parse_refuses_unknown_names() {
    assert_eq!(
        resolve("bob"),
        Err(UserError::UnknownUser {
            name: "bob".to_string()
        })
    );
    assert_eq!(
        resolve("alice:nogroup"),
        Err(UserError::UnknownGroup {
            name: "nogroup".to_string()
        })
    );
    assert!(matches!(resolve(""), Err(UserError::BadFormat { .. })));
    assert!(matches!(
        resolve("alice:"),
        Err(UserError::BadFormat { .. })
    ));
}
