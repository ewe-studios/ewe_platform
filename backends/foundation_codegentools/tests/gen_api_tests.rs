//! Tests for the `gen_api` CLI command builder — the `generate` command's flags
//! and the `cargo fix` post-step they drive (feature 14 E4).

#![cfg(feature = "cli")]

use foundation_codegentools::cli::gen_api::{cargo_fix_args, command};

#[test]
fn cargo_fix_passes_the_profile_through() {
    // E4: the post-step used to always run `cargo fix` in the dev profile. With a
    // profile named, it must reach the command as `--profile`; without one, the
    // flag must be absent so cargo keeps its default.
    let args = cargo_fix_args("foundation_deployment_hetzner", "hetzner", Some("uat"));
    assert_eq!(
        args.join(" "),
        "fix --lib -p foundation_deployment_hetzner --allow-dirty --features hetzner --profile uat"
    );

    let args = cargo_fix_args("foundation_deployment_cloudflare", "cloudflare", None);
    assert!(
        !args.iter().any(|a| a == "--profile"),
        "no --profile when none was named: {args:?}"
    );
}

#[test]
fn generate_declares_a_profile_flag() {
    let cmd = command();
    let matches = cmd
        .try_get_matches_from(["gen_api", "generate", "hetzner", "--profile", "uat"])
        .expect("parses");
    let (_, sub) = matches.subcommand().expect("generate subcommand");
    assert_eq!(
        sub.get_one::<String>("profile").map(String::as_str),
        Some("uat")
    );
}

#[test]
fn cargo_fix_is_opt_in() {
    // Feature 71 E4: the post-step is off unless `--fix` is given.
    let parse = |extra: &[&str]| {
        let mut args = vec!["gen_api", "generate", "hetzner"];
        args.extend_from_slice(extra);
        let matches = command().try_get_matches_from(args).expect("parses");
        let (_, sub) = matches.subcommand().expect("generate subcommand");
        sub.get_flag("fix")
    };
    assert!(!parse(&[]), "cargo fix must not run by default");
    assert!(parse(&["--fix"]), "--fix turns the step on");
}

#[test]
fn generate_declares_a_selection_file_flag() {
    let cmd = command();
    let matches = cmd
        .try_get_matches_from([
            "gen_api",
            "generate",
            "hetzner",
            "--selection-file",
            "codegen.json",
        ])
        .expect("parses");
    let (_, sub) = matches.subcommand().expect("generate subcommand");
    assert_eq!(
        sub.get_one::<String>("selection-file").map(String::as_str),
        Some("codegen.json")
    );
}
