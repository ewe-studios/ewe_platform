//! Check Cloudflare task arguments without contacting an account.
#[cfg(unix)]
#[test]
fn cloudflare_operations_preserve_arguments_and_stop_after_tool_failure() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let tool = root.join("pnpm");
    fs::write(&tool, "#!/bin/sh\nprintf '<%s> ' \"$@\" >> arguments\nprintf '\\n' >> arguments\nexit \"$EXIT_STATUS\"\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str], status: &str| {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", root)
            .env("EXIT_STATUS", status)
            .args(["task", "run"])
            .args(args)
            .status()
            .unwrap()
    };
    for args in [
        &["cf:install"][..],
        &["cf:whoami"],
        &["cf:list"],
        &["cf:d1:create"],
        &["cf:d1:migrate", "local database"],
        &["cf:r2:create", "dev-bucket"],
    ] {
        assert!(run(args, "0").success());
    }
    assert_eq!(run(&["cf:list"], "43").code(), Some(43));
    let calls = fs::read_to_string(root.join("arguments")).unwrap();
    assert_eq!(
        calls.lines().map(str::trim_end).collect::<Vec<_>>(),
        [
            "<install>",
            "<exec> <wrangler> <whoami>",
            "<exec> <wrangler> <d1> <list>",
            "<exec> <wrangler> <r2> <bucket> <list>",
            "<exec> <wrangler> <d1> <create> <foundation-db>",
            "<exec> <wrangler> <d1> <migrations> <apply> <local database> <--local>",
            "<exec> <wrangler> <r2> <bucket> <create> <dev-bucket>",
            "<exec> <wrangler> <d1> <list>",
        ]
    );
}

#[test]
fn account_configuration_reports_only_token_presence() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    for token in [None, Some(""), Some("fixture-credential-value")] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_foundation-ci"));
        cmd.current_dir(dir.path())
            .args(["task", "run", "cf:env"])
            .env_remove("CLOUDFLARE_API_TOKEN")
            .env("CLOUDFLARE_ACCOUNT_ID", "fixture-account")
            .env_remove("DEPLOYMENT_R2_BUCKET");
        if let Some(value) = token {
            cmd.env("CLOUDFLARE_API_TOKEN", value);
        }
        let result = cmd.output().unwrap();
        assert!(result.status.success());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("CLOUDFLARE_ACCOUNT_ID: fixture-account"));
        assert!(text.contains("DEPLOYMENT_R2_BUCKET: not set"));
        let presence = if token == Some("fixture-credential-value") {
            "set (hidden)"
        } else {
            "not set"
        };
        assert!(text.contains(&format!("CLOUDFLARE_API_TOKEN: {presence}")));
        assert!(!text.contains("fixture-credential-value"));
        assert_eq!(result.stderr, b"");
    }
}
