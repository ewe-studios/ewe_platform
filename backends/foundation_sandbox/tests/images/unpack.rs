//! The unpack rows of section 7 (feature 12): the tar reader, the path rules,
//! the whiteouts and the limits. Every test applies a tar to a real temporary
//! directory, so no `chroot` and no root right is needed.

use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::path::Path;

use foundation_sandbox::native::unpack::{kind_from_typeflag, unpack, UnpackError, UnpackLimits};
use foundation_sandbox::shared::unpack::UnpackOptions;

use super::support;

const BLOCK: usize = 512;

/// A hand-built tar archive.
struct TestTar {
    bytes: Vec<u8>,
}

impl TestTar {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn entry(&mut self, header: &[u8; BLOCK], data: &[u8]) {
        self.bytes.extend_from_slice(header);
        self.bytes.extend_from_slice(data);
        let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
        self.bytes.extend(std::iter::repeat_n(0_u8, padding));
    }

    fn file(&mut self, name: &str, body: &[u8], mode: u32) {
        let header = header(name, "", mode, body.len() as u64, b'0', "");
        self.entry(&header, body);
    }

    fn dir(&mut self, name: &str) {
        let header = header(name, "", 0o755, 0, b'5', "");
        self.entry(&header, b"");
    }

    fn symlink(&mut self, name: &str, target: &str) {
        let header = header(name, "", 0o777, 0, b'2', target);
        self.entry(&header, b"");
    }

    fn hard_link(&mut self, name: &str, target: &str) {
        let header = header(name, "", 0o644, 0, b'1', target);
        self.entry(&header, b"");
    }

    fn fifo(&mut self, name: &str) {
        let header = header(name, "", 0o600, 0, b'6', "");
        self.entry(&header, b"");
    }

    fn prefix_file(&mut self, prefix: &str, name: &str, body: &[u8]) {
        let header = header(name, prefix, 0o644, body.len() as u64, b'0', "");
        self.entry(&header, body);
    }

    fn pax_file(&mut self, name: &str, body: &[u8]) {
        // The length prefix includes its own digits, so compute it exactly.
        let record = pax_record("path", name);
        let pax_header = header("PaxHeader", "", 0o644, record.len() as u64, b'x', "");
        self.entry(&pax_header, record.as_bytes());
        let short = header("short", "", 0o644, body.len() as u64, b'0', "");
        self.entry(&short, body);
    }

    fn gnu_long_name(&mut self, name: &str, body: &[u8]) {
        let long_header = header("././@LongLink", "", 0o644, name.len() as u64 + 1, b'L', "");
        let mut data = name.as_bytes().to_vec();
        data.push(0);
        self.entry(&long_header, &data);
        let short = header("short", "", 0o644, body.len() as u64, b'0', "");
        self.entry(&short, body);
    }

    fn finish(mut self) -> Vec<u8> {
        self.bytes.extend(std::iter::repeat_n(0_u8, BLOCK * 2));
        self.bytes
    }
}

/// One PAX record with the correct length prefix.
fn pax_record(key: &str, value: &str) -> String {
    let body = format!(" {key}={value}\n");
    let mut length = body.len() + 1;
    loop {
        let text = format!("{length}{body}");
        if text.len() == length {
            return text;
        }
        length = text.len();
    }
}

/// One 512-byte ustar header.
fn header(name: &str, prefix: &str, mode: u32, size: u64, typeflag: u8, link: &str) -> [u8; BLOCK] {
    let mut block = [0_u8; BLOCK];
    put(&mut block[0..100], name.as_bytes());
    put(&mut block[100..108], &octal(u64::from(mode), 8));
    put(&mut block[108..116], &octal(u64::from(current_uid()), 8));
    put(&mut block[116..124], &octal(u64::from(current_gid()), 8));
    put(&mut block[124..136], &octal(size, 12));
    put(&mut block[136..148], &octal(0, 12));
    block[148..156].fill(b' ');
    block[156] = typeflag;
    put(&mut block[157..257], link.as_bytes());
    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");
    put(&mut block[345..500], prefix.as_bytes());
    let checksum: u64 = block.iter().map(|byte| u64::from(*byte)).sum();
    put(&mut block[148..156], &octal(checksum, 7));
    block[155] = b' ';
    block
}

fn put(field: &mut [u8], bytes: &[u8]) {
    let length = field.len().min(bytes.len());
    field[..length].copy_from_slice(&bytes[..length]);
}

/// One octal numeric field with a trailing zero.
fn octal(value: u64, width: usize) -> Vec<u8> {
    let digits = format!("{value:0width$o}", width = width - 1);
    let mut bytes = digits.into_bytes();
    bytes.push(0);
    bytes
}

fn current_uid() -> u32 {
    // SAFETY: `geteuid` reads the process credential and has no side effect.
    unsafe { libc::geteuid() }
}

fn current_gid() -> u32 {
    // SAFETY: `getegid` reads the process credential and has no side effect.
    unsafe { libc::getegid() }
}

fn limits() -> UnpackLimits {
    UnpackLimits::default()
}

#[test]
fn tar_reader_formats() {
    let dir = support::TempDir::new("formats");
    let mut tar = TestTar::new();
    tar.dir("dir");
    tar.file("dir/file", b"hello", 0o644);
    tar.symlink("link", "dir/file");
    tar.hard_link("hard", "dir/file");
    tar.fifo("fifo");
    tar.prefix_file("longdir", "file", b"prefix");
    tar.pax_file(
        "pax/very/long/name/that/the/ustar/field/cannot/hold/file.txt",
        b"pax",
    );
    tar.gnu_long_name(
        "gnu/very/long/name/that/the/ustar/field/cannot/hold/file.txt",
        b"gnu",
    );
    let archive = tar.finish();
    let stats = unpack(
        archive.as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &limits(),
    )
    .expect("the archive applies");
    assert_eq!(stats.entries, 8, "the eight entry types apply");

    assert!(dir.path().join("dir").is_dir());
    assert_eq!(
        std::fs::read(dir.path().join("dir/file")).expect("the file"),
        b"hello"
    );
    let link = std::fs::symlink_metadata(dir.path().join("link")).expect("the link");
    assert!(link.file_type().is_symlink());
    assert_eq!(
        std::fs::read_link(dir.path().join("link")).expect("the target"),
        Path::new("dir/file")
    );
    let source = std::fs::metadata(dir.path().join("dir/file")).expect("the source");
    let hard = std::fs::metadata(dir.path().join("hard")).expect("the hard link");
    assert_eq!(source.ino(), hard.ino(), "the hard link shares the inode");
    let fifo = std::fs::symlink_metadata(dir.path().join("fifo")).expect("the fifo");
    assert!(fifo.file_type().is_fifo());
    assert_eq!(
        std::fs::read(dir.path().join("longdir/file")).expect("the prefix file"),
        b"prefix"
    );
    assert_eq!(
        std::fs::read(
            dir.path()
                .join("pax/very/long/name/that/the/ustar/field/cannot/hold/file.txt")
        )
        .expect("the PAX file"),
        b"pax"
    );
    assert_eq!(
        std::fs::read(
            dir.path()
                .join("gnu/very/long/name/that/the/ustar/field/cannot/hold/file.txt")
        )
        .expect("the GNU long name file"),
        b"gnu"
    );
    // The reader knows the two device types that an unprivileged test cannot
    // create.
    assert_eq!(kind_from_typeflag(b'3'), Some("char"));
    assert_eq!(kind_from_typeflag(b'4'), Some("block"));
    assert_eq!(kind_from_typeflag(b'S'), Some("sparse"));
}

#[test]
fn unpack_path_rules() {
    let dir = support::TempDir::new("paths");
    let mut tar = TestTar::new();
    tar.file("../x", b"escape", 0o644);
    let error = unpack(
        tar.finish().as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &limits(),
    )
    .expect_err("a `..` part is refused");
    assert!(matches!(error, UnpackError::BadRequest { .. }));

    // An absolute path loses its first `/`.
    let mut tar = TestTar::new();
    tar.file("/abs", b"absolute", 0o644);
    unpack(
        tar.finish().as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &limits(),
    )
    .expect("an absolute path is made relative");
    assert_eq!(
        std::fs::read(dir.path().join("abs")).expect("the file"),
        b"absolute"
    );

    // A symbolic link to `/` stays inside the root, as it does in a chroot.
    let mut tar = TestTar::new();
    tar.symlink("d", "/");
    tar.file("d/y", b"inside", 0o644);
    unpack(
        tar.finish().as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &limits(),
    )
    .expect("the target is resolved inside the root");
    assert_eq!(
        std::fs::read(dir.path().join("y")).expect("the file inside the root"),
        b"inside"
    );
}

#[test]
fn unpack_whiteouts() {
    let dir = support::TempDir::new("whiteouts");
    std::fs::create_dir_all(dir.path().join("a")).expect("the directory");
    std::fs::write(dir.path().join("a/b"), b"old").expect("the file");
    std::fs::create_dir_all(dir.path().join("c")).expect("the directory");
    std::fs::write(dir.path().join("c/d"), b"old").expect("the file");
    let mut tar = TestTar::new();
    tar.file("a/.wh.b", b"", 0o644);
    tar.file("c/.wh..wh..opq", b"", 0o644);
    tar.file("c/e", b"new", 0o644);
    let options = UnpackOptions {
        oci_whiteouts: true,
        skip: Vec::new(),
    };
    unpack(tar.finish().as_slice(), dir.path(), &options, &limits()).expect("the archive applies");
    assert!(!dir.path().join("a/b").exists(), "the whiteout removes b");
    assert!(
        !dir.path().join("c/d").exists(),
        "the opaque whiteout removes d"
    );
    assert_eq!(
        std::fs::read(dir.path().join("c/e")).expect("the new file"),
        b"new"
    );
}

#[test]
fn unpack_limits() {
    let dir = support::TempDir::new("limits");
    let mut tar = TestTar::new();
    tar.file("big", &[0_u8; 8], 0o644);
    let small_bytes = UnpackLimits {
        max_bytes: 4,
        ..limits()
    };
    let error = unpack(
        tar.finish().as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &small_bytes,
    )
    .expect_err("the byte limit stops the unpack");
    assert!(matches!(error, UnpackError::Limit { .. }));

    let mut tar = TestTar::new();
    tar.file("one", b"a", 0o644);
    tar.file("two", b"b", 0o644);
    let one_entry = UnpackLimits {
        max_entries: 1,
        ..limits()
    };
    let error = unpack(
        tar.finish().as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &one_entry,
    )
    .expect_err("the entry limit stops the unpack");
    assert!(matches!(error, UnpackError::Limit { .. }));

    let mut tar = TestTar::new();
    tar.file("longname", b"x", 0o644);
    let short_path = UnpackLimits {
        max_path_bytes: 4,
        ..limits()
    };
    let error = unpack(
        tar.finish().as_slice(),
        dir.path(),
        &UnpackOptions::default(),
        &short_path,
    )
    .expect_err("the path limit stops the unpack");
    assert!(matches!(error, UnpackError::Limit { .. }));
}

#[test]
fn unpack_skip() {
    let dir = support::TempDir::new("skip");
    let mut tar = TestTar::new();
    tar.file("keep", b"keep", 0o644);
    tar.file("drop", b"drop", 0o644);
    let options = UnpackOptions {
        oci_whiteouts: false,
        skip: vec!["drop".to_string()],
    };
    unpack(tar.finish().as_slice(), dir.path(), &options, &limits()).expect("the archive applies");
    assert!(dir.path().join("keep").exists());
    assert!(!dir.path().join("drop").exists(), "the skip drops the path");
}

#[cfg(feature = "artifacts")]
mod property {
    use proptest::prelude::*;

    use foundation_sandbox::native::unpack::{unpack, UnpackLimits};
    use foundation_sandbox::shared::unpack::UnpackOptions;

    use super::support;

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: 16,
            rng_seed: proptest::test_runner::RngSeed::Fixed(0x5eed),
            ..ProptestConfig::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn tar_reader_property(files in prop::collection::vec(
            (0_u8..4, prop::collection::vec(any::<u8>(), 0..64)),
            1..4,
        )) {
            let dir = support::TempDir::new("property");
            let mut builder = tar::Builder::new(Vec::new());
            let mut expected = Vec::new();
            for (index, (kind, body)) in files.iter().enumerate() {
                let name = format!("f{index}");
                let mut header = tar::Header::new_gnu();
                header.set_size(body.len() as u64);
                header.set_mode(0o644);
                // SAFETY: `geteuid` and `getegid` read the process credential
                // and have no side effect.
                header.set_uid(u64::from(unsafe { libc::geteuid() }));
                // SAFETY: as above.
                header.set_gid(u64::from(unsafe { libc::getegid() }));
                header.set_mtime(0);
                if *kind == 0 {
                    header.set_entry_type(tar::EntryType::Regular);
                    builder
                        .append_data(&mut header, &name, body.as_slice())
                        .expect("the entry appends");
                    expected.push((name, body.clone()));
                } else {
                    // A symbolic link with a short target.
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_size(0);
                    builder
                        .append_link(&mut header, &name, "f0")
                        .expect("the link appends");
                }
            }
            let archive = builder.into_inner().expect("the archive ends");
            let stats = unpack(
                archive.as_slice(),
                dir.path(),
                &UnpackOptions::default(),
                &UnpackLimits::default(),
            )
            .expect("the generated archive applies");
            prop_assert_eq!(stats.entries, files.len() as u64);
            for (name, body) in expected {
                prop_assert_eq!(
                    std::fs::read(dir.path().join(&name)).expect("the file"),
                    body
                );
            }
        }
    }
}
