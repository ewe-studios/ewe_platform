/// Example: LD_PRELOAD VFS Shim.
///
/// This example demonstrates how to use the LD_PRELOAD VFS shim — a shared library
/// that intercepts libc filesystem calls and redirects virtual paths through the
/// real VFS stack (OverlayFileSystem with pluggable delta stores).
///
/// The shim is built as a `cdylib` target, not a normal binary. This example
/// compiles it, then demonstrates usage scenarios with C test programs.
///
/// ## What it does
///
/// The shim sits between your application and libc. When your app calls `open()`,
/// `read()`, `write()`, `stat()`, etc., the shim checks if the path matches a
/// configured prefix (e.g., `/virtual`). If it does, the call goes through the
/// VFS stack. If not, it passes through to the real filesystem.
///
/// ## Scenarios
///
/// 1. **Sandboxed development** — Run build tools (`gcc`, `make`, `python`)
///    against a virtual filesystem without modifying source paths.
///
/// 2. **Testing without side effects** — Write to `/virtual/` instead of real
///    paths. All writes go to the delta store (memory, sqlite, directory, etc.).
///    Restarting clears memory delta; sqlite persists across runs.
///
/// 3. **Cloud-synced deltas** — Use `turso` or `d1` delta stores to share
///    virtual filesystem state across machines.
///
/// 4. **Object storage as VFS** — Use `r2` to store virtual files in S3-
///    compatible cloud storage.
///
/// 5. **Legacy app compatibility** — Intercept hardcoded paths in apps you
///    can't modify. Redirect `/opt/app/data` → virtual overlay with custom
///    content.

use std::process::Command;

fn main() {
    println!("=== LD_PRELOAD VFS Shim Example ===\n");

    // The shim is built as a cdylib — find it
    let lib = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.parent().map(|p| p.to_path_buf())).flatten())
        .and_then(|target_dir| {
            // Find the .so in target/debug
            std::fs::read_dir(&target_dir).ok().and_then(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .find(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with("libfoundation_nativeapis")
                            && e.file_name().to_string_lossy().ends_with(".so")
                    })
                    .map(|e| e.path())
            })
        });

    let lib_path = match lib {
        Some(p) => p,
        None => {
            println!("Build the shim first:");
            println!("  cargo build -p foundation_nativeapis --features vfs-preload\n");
            println!("The shim is: target/debug/libfoundation_nativeapis.so");
            println!();
            println!("Then use it like this:");
            println!();
            println!("  # Quick test with memory delta:");
            println!("  LD_PRELOAD=target/debug/libfoundation_nativeapis.so \\");
            println!("    FOUNDATION_VFS_PREFIX=/virtual \\");
            println!("    /bin/bash");
            println!();
            println!("  # Persistent delta with sqlite:");
            println!("  LD_PRELOAD=target/debug/libfoundation_nativeapis.so \\");
            println!("    FOUNDATION_VFS_PREFIX=/virtual \\");
            println!("    FOUNDATION_VFS_DELTA=sqlite \\");
            println!("    FOUNDATION_VFS_DELTA_PATH=/tmp/vfs-delta.db \\");
            println!("    /bin/bash");
            println!();
            println!("  # Delta stored in a real directory:");
            println!("  LD_PRELOAD=target/debug/libfoundation_nativeapis.so \\");
            println!("    FOUNDATION_VFS_PREFIX=/virtual \\");
            println!("    FOUNDATION_VFS_DELTA=dir \\");
            println!("    FOUNDATION_VFS_DELTA_PATH=/tmp/vfs-delta \\");
            println!("    /bin/bash");
            println!();
            println!("  # Overlay a real directory as base:");
            println!("  LD_PRELOAD=target/debug/libfoundation_nativeapis.so \\");
            println!("    FOUNDATION_VFS_PREFIX=/virtual \\");
            println!("    FOUNDATION_VFS_ROOT=/path/to/project \\");
            println!("    FOUNDATION_VFS_DELTA=memory \\");
            println!("    /bin/bash");
            return;
        }
    };

    let lib_str = lib_path.to_string_lossy().into_owned();
    let tmp_dir = std::env::temp_dir();
    let root = tmp_dir.join("vfs-shim-example");
    std::fs::create_dir_all(&root).ok();

    // Create a test C program
    let c_code = r#"
#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <sys/stat.h>

int main(int argc, char *argv[]) {
    // 1. Read a file from the base filesystem (real directory)
    printf("=== Scenario 1: Read from base filesystem ===\n");
    int fd = open("/base/readme.txt", O_RDONLY);
    if (fd >= 0) {
        char buf[256];
        int n = read(fd, buf, sizeof(buf) - 1);
        if (n > 0) { buf[n] = '\0'; printf("  /base/readme.txt: %s\n", buf); }
        close(fd);
    } else {
        printf("  /base/readme.txt: not found (ok if first run)\n");
    }

    // 2. Write to virtual filesystem (goes to delta store)
    printf("\n=== Scenario 2: Write to virtual filesystem ===\n");
    fd = open("/virtual/hello.txt", O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd >= 0) {
        const char *msg = "Hello from the VFS shim!\n";
        int n = write(fd, msg, strlen(msg));
        printf("  Wrote %d bytes to /virtual/hello.txt\n", n);
        close(fd);
    } else {
        printf("  FAILED to open /virtual/hello.txt for write\n");
    }

    // 3. Read it back
    printf("\n=== Scenario 3: Read from virtual filesystem ===\n");
    fd = open("/virtual/hello.txt", O_RDONLY);
    if (fd >= 0) {
        char buf[256];
        int n = read(fd, buf, sizeof(buf) - 1);
        if (n > 0) { buf[n] = '\0'; printf("  /virtual/hello.txt: %s\n", buf); }
        close(fd);
    } else {
        printf("  FAILED to open /virtual/hello.txt for read\n");
    }

    // 4. Stat a virtual file
    printf("\n=== Scenario 4: Stat virtual file ===\n");
    struct stat st;
    if (stat("/virtual/hello.txt", &st) == 0) {
        printf("  size: %ld bytes\n", (long)st.st_size);
        printf("  mode: %o\n", st.st_mode & 0777);
    } else {
        printf("  stat failed\n");
    }

    // 5. Non-virtual path passes through
    printf("\n=== Scenario 5: Non-virtual path passthrough ===\n");
    fd = open("/etc/hostname", O_RDONLY);
    if (fd >= 0) {
        char buf[256];
        int n = read(fd, buf, sizeof(buf) - 1);
        if (n > 0) { buf[n] = '\0'; printf("  /etc/hostname: %s\n", buf); }
        close(fd);
    } else {
        printf("  /etc/hostname: not found\n");
    }

    printf("\n=== Done ===\n");
    return 0;
}
"#;

    let c_path = root.join("test.c");
    let exe_path = root.join("test");
    std::fs::write(&c_path, c_code).ok();

    // Compile the test program
    let status = Command::new("gcc")
        .arg(&c_path)
        .arg("-o")
        .arg(&exe_path)
        .status()
        .expect("gcc failed");

    if !status.success() {
        println!("gcc failed to compile test program");
        return;
    }

    println!("Shim: {lib_str}");
    println!("Test: {}\n", exe_path.display());

    // Run with different delta backends

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Scenario: Memory Delta (default, ephemeral)             │");
    println!("│ Use case: Fast, no persistence, good for sandboxing    │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    Command::new(&exe_path)
        .env("LD_PRELOAD", &lib_str)
        .env("FOUNDATION_VFS_PREFIX", "/virtual:/base")
        .env("FOUNDATION_VFS_ROOT", &root)
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            print!("{stdout}");
            if !stderr.is_empty() { eprintln!("{stderr}"); }
        })
        .ok();

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Scenario: SQLite Delta (persistent, local)              │");
    println!("│ Use case: Survives restarts, low latency               │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    Command::new(&exe_path)
        .env("LD_PRELOAD", &lib_str)
        .env("FOUNDATION_VFS_PREFIX", "/virtual:/base")
        .env("FOUNDATION_VFS_ROOT", &root)
        .env("FOUNDATION_VFS_DELTA", "sqlite")
        .env("FOUNDATION_VFS_DELTA_PATH", root.join("vfs-delta.db"))
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            print!("{stdout}");
            if !stderr.is_empty() { eprintln!("{stderr}"); }
        })
        .ok();

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Scenario: Directory Delta (human-readable, persistent)  │");
    println!("│ Use case: Inspect delta files directly with cat/ls     │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    Command::new(&exe_path)
        .env("LD_PRELOAD", &lib_str)
        .env("FOUNDATION_VFS_PREFIX", "/virtual:/base")
        .env("FOUNDATION_VFS_ROOT", &root)
        .env("FOUNDATION_VFS_DELTA", "dir")
        .env("FOUNDATION_VFS_DELTA_PATH", root.join("vfs-delta-dir"))
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            print!("{stdout}");
            if !stderr.is_empty() { eprintln!("{stderr}"); }
        })
        .ok();

    // Show delta directory contents
    let delta_dir = root.join("vfs-delta-dir");
    if delta_dir.exists() {
        println!("\nDelta directory contents:");
        if let Ok(entries) = std::fs::read_dir(&delta_dir) {
            for e in entries.flatten() {
                println!("  {} ({:?})", e.file_name().to_string_lossy(), e.file_type().ok());
                if let Ok(content) = std::fs::read_to_string(e.path()) {
                    println!("    {:?}", content);
                }
            }
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&root);

    println!("\n=== Configuration Reference ===\n");
    println!("Environment variables:");
    println!("  FOUNDATION_VFS_PREFIX   — Colon-separated virtual path prefixes (e.g., /virtual)");
    println!("  FOUNDATION_VFS_ROOT     — Base directory for NativeFs overlay (default: .)");
    println!("  FOUNDATION_VFS_DELTA    — Delta backend (memory|sqlite|turso|dir|d1|r2)");
    println!("  FOUNDATION_VFS_DELTA_PATH — Path for persistent delta stores");
    println!("  FOUNDATION_VFS_TURSO_URL — Turso connection URL (for turso delta)");
    println!("  FOUNDATION_VFS_TURSO_TOKEN — Turso auth token (for turso delta)");
    println!("  FOUNDATION_VFS_D1_ACCOUNT_ID — Cloudflare account ID (for d1 delta)");
    println!("  FOUNDATION_VFS_D1_API_TOKEN — Cloudflare API token (for d1 delta)");
    println!("  FOUNDATION_VFS_D1_DATABASE_ID — D1 database ID (for d1 delta)");
    println!("  FOUNDATION_VFS_R2_ACCOUNT_ID — Cloudflare account ID (for r2 delta)");
    println!("  FOUNDATION_VFS_R2_ACCESS_KEY — R2 access key (for r2 delta)");
    println!("  FOUNDATION_VFS_R2_SECRET_KEY — R2 secret key (for r2 delta)");
    println!("  FOUNDATION_VFS_R2_BUCKET — R2 bucket name (for r2 delta)");
}
