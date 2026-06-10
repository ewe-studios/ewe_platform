#![cfg(all(feature = "vfs-preload", target_os = "linux"))]

use std::process::Command;

fn find_shim_so() -> String {
    // Test binary is in target/debug/deps/
    // Shim .so is in target/debug/
    let target = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf())) // deps/
        .and_then(|p| p.parent().map(|p| p.to_path_buf())) // debug/
        .expect("could not find target directory");
    for entry in std::fs::read_dir(&target).unwrap().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("libfoundation_nativeapis") && name.ends_with(".so") {
            return entry.path().to_string_lossy().to_string();
        }
    }
    panic!("shim .so not found in {:?}", target);
}

#[test]
fn test_shim_writes_to_virtual_path() {
    let so = find_shim_so();
    let tmpdir = std::env::temp_dir();
    let c_path = tmpdir.join("shim_test.c");
    let exe_path = tmpdir.join("shim_test");
    std::fs::write(&c_path, r#"
#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
int main() {
    int fd = open("/virtual/test.txt", O_WRONLY|O_CREAT|O_TRUNC, 0644);
    if (fd < 0) return 1;
    write(fd, "hello\n", 6);
    close(fd);
    fd = open("/virtual/test.txt", O_RDONLY);
    if (fd < 0) return 2;
    char buf[32];
    int n = read(fd, buf, sizeof(buf)-1);
    buf[n] = 0;
    printf("%s", buf);
    close(fd);
    return 0;
}
"#).unwrap();

    let status = std::process::Command::new("gcc")
        .arg(&c_path).arg("-o").arg(&exe_path)
        .status().unwrap();
    assert!(status.success(), "gcc failed");

    let output = std::process::Command::new(&exe_path)
        .env("LD_PRELOAD", &so)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "memory")
        .output()
        .expect("command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "exit: {:?}, stdout: {}, stderr: {}",
        output.status, stdout, String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("hello"), "stdout: {}", stdout);

    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&exe_path);
}

#[test]
fn test_shim_real_path_passthrough() {
    let so = find_shim_so();
    let output = Command::new("/bin/sh")
        .env("LD_PRELOAD", &so)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "memory")
        .arg("-c")
        .arg("cat /etc/hostname")
        .output()
        .expect("command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "should read real file, got: {}", stdout);
}
