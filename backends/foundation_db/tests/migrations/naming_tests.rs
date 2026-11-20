//! Migration file-name parsing.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_db::{parse_migration_name, MigrationProblem};

#[test]
fn a_valid_name_gives_a_number_and_a_name() {
    let parsed = parse_migration_name("0001_create_users.sql", "origin", 4).unwrap();
    assert_eq!(parsed.number, 1);
    assert_eq!(parsed.name, "create_users");
}

#[test]
fn leading_zeros_do_not_change_the_number() {
    let four = parse_migration_name("0001_create_users.sql", "origin", 0).unwrap();
    let one = parse_migration_name("1_create_users.sql", "origin", 0).unwrap();
    assert_eq!(four.number, one.number);
    assert_eq!(four.number, 1);
}

#[test]
fn a_short_number_is_reported() {
    let problem = parse_migration_name("042_users.sql", "origin", 4).unwrap_err();
    assert!(matches!(
        problem,
        MigrationProblem::ShortNumber { digits: 3, .. }
    ));
}

#[test]
fn a_number_past_9999_is_valid() {
    let parsed = parse_migration_name("10000_many.sql", "origin", 4).unwrap();
    assert_eq!(parsed.number, 10_000);
}

#[test]
fn bad_names_are_reported() {
    for name in [
        "create_users.sql",
        "01-users.sql",
        "0001_Create.sql",
        "0001_.sql",
        "0001_create-users.sql",
        "0000_zero.sql",
        "0001_create_users.txt",
    ] {
        let problem =
            parse_migration_name(name, "origin", 0).expect_err(&format!("{name} must fail"));
        assert!(
            matches!(problem, MigrationProblem::BadName { .. }),
            "{name} gave {problem:?}"
        );
    }
}
