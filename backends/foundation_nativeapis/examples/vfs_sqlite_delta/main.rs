/// Example: LD_PRELOAD VFS Shim with SQLite Delta Store.
///
/// Demonstrates persistent virtual filesystem backed by SQLite,
/// loaded via LD_PRELOAD. Writes survive process restart.
///
/// ```bash
/// cargo run -p foundation_nativeapis --features "vfs-preload,vfs-sqlite" --example vfs_sqlite_delta
/// ```

use std::process::Command;

const TEST_C: &str = r#"
#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <dirent.h>
#include <sys/stat.h>

int main(int argc, char *argv[]) {
    const char *mode = argc > 1 ? argv[1] : "write";
    const char *path = "/virtual/persistent.txt";

    if (strcmp(mode, "write") == 0) {
        int fd = open(path, O_WRONLY | O_CREAT | O_APPEND, 0644);
        if (fd < 0) { perror("open for write"); return 1; }
        const char *msg = "Hello from SQLite delta store!\n";
        int n = write(fd, msg, strlen(msg));
        printf("Wrote %d bytes to %s\n", n, path);
        close(fd);
    } else if (strcmp(mode, "read") == 0) {
        int fd = open(path, O_RDONLY);
        if (fd < 0) { perror("open for read"); return 1; }
        char buf[512];
        int n = read(fd, buf, sizeof(buf) - 1);
        if (n > 0) { buf[n] = '\0'; printf("Read from %s:\n  %s", path, buf); }
        close(fd);
    } else if (strcmp(mode, "stat") == 0) {
        struct stat st;
        if (stat(path, &st) == 0) {
            printf("%s: size=%ld bytes, mode=%o\n", path, (long)st.st_size, st.st_mode & 0777);
        } else { perror("stat"); }
    } else if (strcmp(mode, "multi") == 0) {
        const char *files[] = {
            "/virtual/notes.txt", "/virtual/config.ini", NULL
        };
        for (int i = 0; files[i] != NULL; i++) {
            int fd = open(files[i], O_WRONLY | O_CREAT | O_TRUNC, 0644);
            if (fd >= 0) {
                char msg[128];
                snprintf(msg, sizeof(msg), "Content of %s via SQLite delta\n", files[i]);
                write(fd, msg, strlen(msg));
                printf("  wrote %s\n", files[i]);
                close(fd);
            } else { printf("  failed %s\n", files[i]); }
        }
    }
    return 0;
}
"#;

fn find_shim() -> String {
    let target = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .expect("could not find target directory");

    for dir in [&target] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("libfoundation_nativeapis") && name.ends_with(".so") {
                    return e.path().to_string_lossy().to_string();
                }
            }
        }
    }

    if let Some(parent) = target.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for e in entries.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("libfoundation_nativeapis") && name.ends_with(".so") {
                    return e.path().to_string_lossy().to_string();
                }
            }
        }
    }

    panic!(
        "Could not find libfoundation_nativeapis.so in {}. \
         Build with: cargo build -p foundation_nativeapis --features vfs-preload,vfs-sqlite",
        target.display()
    );
}

fn run(shim: &str, db: &str, args: &[&str]) {
    let output = Command::new(args[0])
        .args(&args[1..])
        .env("LD_PRELOAD", shim)
        .env("FOUNDATION_VFS_PREFIX", "/virtual")
        .env("FOUNDATION_VFS_DELTA", "sqlite")
        .env("FOUNDATION_VFS_DELTA_PATH", db)
        .output()
        .expect("command failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{}", stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        if line.contains("[vfs-shim]") {
            eprintln!("{}", line);
        }
    }
}

fn main() {
    println!("=== LD_PRELOAD VFS Shim: SQLite Delta Store ===\n");

    let shim = find_shim();
    let db = std::env::temp_dir().join("vfs-sqlite-example.db");
    let db_str = db.to_string_lossy();

    // Clean previous run
    let _ = std::fs::remove_file(&db);

    // Compile test program
    let out_dir = std::env::temp_dir().join("vfs-sqlite-example");
    std::fs::create_dir_all(&out_dir).ok();
    let c_path = out_dir.join("test.c");
    let exe_path = out_dir.join("test");
    std::fs::write(&c_path, TEST_C).expect("write test.c");

    let status = Command::new("gcc")
        .arg(&c_path)
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("gcc failed");

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        panic!("gcc failed:\n{stderr}");
    }

    println!("Shim: {shim}");
    println!("SQLite: {db_str}\n");

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 1: Write virtual files (SQLite delta)              │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    run(&shim, &db_str, &[exe_path.to_str().unwrap(), "write"]);

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 2: Read them back (same process)                   │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    run(&shim, &db_str, &[exe_path.to_str().unwrap(), "read"]);

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 3: Write multiple files                            │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    run(&shim, &db_str, &[exe_path.to_str().unwrap(), "multi"]);

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 4: Simulate restart (new process, same DB)         │");
    println!("│ This proves persistence — data survives process exit   │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    run(&shim, &db_str, &[exe_path.to_str().unwrap(), "read"]);

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 5: Stat a virtual file                             │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    run(&shim, &db_str, &[exe_path.to_str().unwrap(), "stat"]);

    // Show SQLite DB
    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ SQLite database on disk                                 │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    if db.exists() {
        let size = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
        let display = if size > 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else {
            format!("{size} bytes")
        };
        println!("  {display}  {db_str}");
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_file(&db);

    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Try with Turso (remote libSQL)                          │");
    println!("│                                                         │");
    println!("│  export FOUNDATION_VFS_TURSO_URL=libsql://your-db...   │");
    println!("│  export FOUNDATION_VFS_TURSO_TOKEN=your-token          │");
    println!("│  FOUNDATION_VFS_DELTA=turso                            │");
    println!("└─────────────────────────────────────────────────────────┘");
}
