/// Example: Turso Delta Store (programmatic API).
///
/// Demonstrates how to use Turso (distributed libSQL) as a VFS delta store.
/// Files written through the overlay are stored in the remote Turso database,
/// making them accessible from any machine with the same database.
///
/// Requires:
///   - A Turso database (https://turso.tech)
///   - `vfs-turso` feature enabled
///
/// ```bash
/// # Without credentials: shows setup instructions
/// cargo run -p foundation_nativeapis --features vfs-turso --example vfs_turso_delta
///
/// # With credentials:
/// export FOUNDATION_VFS_TURSO_URL=libsql://your-database.turso.io
/// export FOUNDATION_VFS_TURSO_TOKEN=your-auth-token
/// cargo run -p foundation_nativeapis --features vfs-turso --example vfs_turso_delta
/// ```

fn main() {
    println!("=== Turso Delta Store Example ===\n");

    let url = std::env::var("FOUNDATION_VFS_TURSO_URL").ok();
    let token = std::env::var("FOUNDATION_VFS_TURSO_TOKEN").ok();

    if url.is_none() || token.is_none() {
        print_setup_instructions();
        return;
    }

    let url = url.unwrap();
    let _token = token.unwrap();

    println!("Connecting to Turso: {url}\n");

    // TursoDelta requires valtron runtime — not available in examples.
    // Show the programmatic API pattern instead.
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Turso Connection                                        │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    println!("Turso URL:  {url}");
    println!("Token:      {}...", &_token.chars().take(8).collect::<String>());
    println!();

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Programmatic API Pattern                                │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    println!("To use Turso as a VFS delta store in your code:");
    println!();
    println!("  use foundation_nativeapis::shared::vfs::{{");
    println!("      turso_delta::TursoDelta,");
    println!("      overlay_fs::OverlayFileSystem,");
    println!("      native_fs::NativeFs,");
    println!("      VfsFileSystem,");
    println!("  }};");
    println!();
    println!("  // Create overlay with Turso delta");
    println!("  let base = NativeFs::new(\"/path/to/project\").unwrap();");
    println!(r#"  let delta = TursoDelta::new("{url}", Some("your-token")).unwrap();"#);
    println!("  let overlay = OverlayFileSystem::new(base, delta);");
    println!();
    println!("  // All writes go to Turso — accessible from any machine");
    println!(r#"  overlay.write_file("/virtual/file.txt", b"hello").unwrap();"#);
    println!(r#"  let data = overlay.read_file("/virtual/file.txt").unwrap();"#);
    println!();
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ LD_PRELOAD Shim (future)                                │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    println!("Turso support through the LD_PRELOAD shim requires initializing");
    println!("the valtron runtime in the shim process. This is a known");
    println!("limitation — tracked in the feature spec.\n");

    println!("  # When available:");
    println!("  LD_PRELOAD=target/debug/libfoundation_nativeapis.so \\");
    println!("    FOUNDATION_VFS_PREFIX=/virtual \\");
    println!("    FOUNDATION_VFS_DELTA=turso \\");
    println!("    FOUNDATION_VFS_TURSO_URL={url} \\");
    println!("    FOUNDATION_VFS_TURSO_TOKEN={_token} \\");
    println!("    /bin/bash");
}

fn print_setup_instructions() {
    println!("Turso credentials not set.\n");
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 1: Install Turso CLI                               │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    println!("  curl -sSf https://getting.turso.sh | sh");
    println!("  turso auth login\n");

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 2: Create a database                               │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    println!("  turso db create my-vfs\n");

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 3: Get connection details                          │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    println!("  turso db show my-vfs --url");
    println!("  turso db tokens create my-vfs\n");

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 4: Set environment variables                       │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    println!("  export FOUNDATION_VFS_TURSO_URL=libsql://my-vfs-user.turso.io");
    println!("  export FOUNDATION_VFS_TURSO_TOKEN=eyJhbG...\n");

    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Step 5: Run the example                                 │");
    println!("└─────────────────────────────────────────────────────────┘\n");
    println!("  cargo run -p foundation_nativeapis \\");
    println!("    --features vfs-turso \\");
    println!("    --example vfs_turso_delta\n");
}
