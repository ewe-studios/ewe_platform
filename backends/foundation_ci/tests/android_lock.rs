//! Verify vendor checksums and preserve the input lock when one archive differs.
#[cfg(unix)]
#[test]
fn android_lock_updates_only_after_all_archives_pass_verification() {
    use sha2::Digest;
    use std::{fmt::Write as _, fs, os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(
        root.join("mise.toml"),
        "[tools]\n'http:android-sdk'={version='test'}\n[env]\n",
    )
    .unwrap();
    let platforms = ["linux-x64", "macos-arm64", "macos-x64", "windows-x64"];
    let data = b"archive fixture";
    let digest = hex::encode(sha1::Sha1::digest(data));
    let sha = hex::encode(sha2::Sha256::digest(data));
    let mut doc = String::from("[[tools.'http:android-sdk']]\nversion='test'\n");
    let mut xml = String::from("<sdk><package><archives>");
    for platform in platforms {
        let file = format!("tools-{platform}.zip");
        fs::write(root.join(&file), data).unwrap();
        xml.push_str(&format!("<archive><complete><size>{}</size><checksum type='sha1'>{digest}</checksum><url>{file}</url></complete></archive>",data.len()).repeat(2));
        writeln!(doc,"\n[tools.'http:android-sdk'.'platforms.{platform}']\nurl = \"https://dl.google.com/android/repository/{file}\"").unwrap();
    }
    xml.push_str("</archives></package></sdk>");
    fs::write(root.join("repository2-3.xml"), xml).unwrap();
    let lock = root.join("mise.lock");
    fs::write(&lock, &doc).unwrap();
    let curl = root.join("curl");
    fs::write(
        &curl,
        r#"#!/bin/sh
while [ "$1" != --url ]; do shift; done
shift
echo "$1" >> urls
cat "${1##*/}"
"#,
    )
    .unwrap();
    fs::set_permissions(curl, fs::Permissions::from_mode(0o755)).unwrap();
    let paths = std::iter::once(root.to_owned())
        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
        .collect::<Vec<_>>();
    let path = std::env::join_paths(paths).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
            .current_dir(root)
            .env("PATH", &path)
            .arg("toolchain-android-lock")
            .output()
            .unwrap()
    };
    fs::write(root.join("tools-windows-x64.zip"), b"archive corrupt").unwrap();
    assert!(!run().status.success());
    assert_eq!(fs::read_to_string(&lock).unwrap(), doc);
    fs::write(root.join("tools-windows-x64.zip"), data).unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let after = fs::read_to_string(&lock).unwrap();
    assert_eq!(
        after,
        doc.replace("url = ", &format!("checksum = \"sha256:{sha}\"\nurl = "))
    );
    let log = fs::read(root.join("urls")).unwrap();
    assert!(run().status.success());
    assert_eq!(log, fs::read(root.join("urls")).unwrap());
}
