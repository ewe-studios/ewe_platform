//! The `newc` archive writer.
//!
//! The Linux kernel reads an uncompressed `newc` archive as an initramfs. For
//! each entry the format has the 6 bytes `070701`, then 13 fields of 8
//! upper-case hexadecimal digits, then the name with its zero byte, then padding
//! to a multiple of 4, then the data, then padding to a multiple of 4.
//!
//! Every field that would change between two runs is fixed: the inode is the
//! entry index from 1, the modification time is 0 and the device numbers are 0.
//! So the same input gives the same bytes (invariant I6).

use std::fmt::Write as _;
use std::io::Write;

/// One entry of a `newc` archive.
#[derive(Clone, Copy, Debug)]
pub struct CpioEntry<'a> {
    /// The path in the archive. It has no leading slash.
    pub path: &'a str,
    /// The mode, with its file type bits (for example `0o040755`).
    pub mode: u32,
    /// The file data. A directory has no data.
    pub data: &'a [u8],
}

/// The mode of a directory: a directory with mode `0755`.
pub const DIR_MODE: u32 = 0o040_755;

/// The mode of an executable file.
pub const FILE_MODE: u32 = 0o100_755;

/// The number of padding bytes that make `length` a multiple of `align`.
fn padding(length: usize, align: usize) -> usize {
    (align - (length % align)) % align
}

/// The twelve-byte magic plus thirteen eight-digit fields.
const HEADER_LEN: usize = 6 + 13 * 8;

/// The trailer entry name.
const TRAILER: &str = "TRAILER!!!";

/// Writes one `newc` header and its fields.
fn write_header(
    out: &mut impl Write,
    inode: u32,
    mode: u32,
    size: usize,
    name_size: usize,
) -> std::io::Result<()> {
    let mut header = String::with_capacity(HEADER_LEN);
    header.push_str("070701");
    // The fields, in order: inode, mode, user ID, group ID, link count,
    // modification time, file size, device major, device minor, device-node
    // major, device-node minor, name size, check sum.
    let is_dir = mode & 0o170_000 == 0o040_000;
    let link_count = if is_dir { 2 } else { 1 };
    for value in [
        inode,
        mode,
        0,
        0,
        link_count,
        0,
        u32::try_from(size).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a newc entry is larger than 4 GiB",
            )
        })?,
        0,
        0,
        0,
        0,
        u32::try_from(name_size).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a newc name is longer than 4 GiB",
            )
        })?,
        0,
    ] {
        let _ = write!(header, "{value:08X}");
    }
    out.write_all(header.as_bytes())
}

/// Writes one entry: the header, the name, the padding, the data and the
/// padding.
fn write_entry(out: &mut impl Write, inode: u32, entry: &CpioEntry<'_>) -> std::io::Result<()> {
    if entry.path.contains('\0') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "a newc name holds a zero byte",
        ));
    }
    let name_size = entry.path.len() + 1;
    write_header(out, inode, entry.mode, entry.data.len(), name_size)?;
    out.write_all(entry.path.as_bytes())?;
    out.write_all(&[0u8])?;
    out.write_all(&vec![0u8; padding(HEADER_LEN + name_size, 4)])?;
    out.write_all(entry.data)?;
    out.write_all(&vec![0u8; padding(entry.data.len(), 4)])
}

/// Writes a `newc` archive: the entries, then the `TRAILER!!!` entry, then zero
/// bytes up to a multiple of 512.
///
/// The same entries always give the same bytes. The modification time field is
/// 0, and every other variable field follows a rule.
///
/// # Errors
///
/// An input or output error, or [`std::io::ErrorKind::InvalidInput`] for a name
/// with a zero byte or an entry larger than 4 GiB.
pub fn write_newc(entries: &[CpioEntry<'_>], out: &mut impl Write) -> std::io::Result<()> {
    let mut total = 0usize;
    for (index, entry) in entries.iter().enumerate() {
        let inode = u32::try_from(index + 1).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "too many newc entries")
        })?;
        write_entry(out, inode, entry)?;
        let name_size = entry.path.len() + 1;
        total += HEADER_LEN
            + name_size
            + padding(HEADER_LEN + name_size, 4)
            + entry.data.len()
            + padding(entry.data.len(), 4);
    }
    let trailer = CpioEntry {
        path: TRAILER,
        mode: 0,
        data: &[],
    };
    let inode = u32::try_from(entries.len() + 1).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "too many newc entries")
    })?;
    write_entry(out, inode, &trailer)?;
    let name_size = TRAILER.len() + 1;
    total += HEADER_LEN + name_size + padding(HEADER_LEN + name_size, 4);
    out.write_all(&vec![0u8; padding(total, 512)])
}

/// The entries of the slave initramfs: the four mount points and the slave as
/// `/init`.
///
/// The modification time of every entry is 0, so the same slave file gives the
/// same archive (invariant I6).
#[must_use]
pub fn initramfs_entries(slave: &[u8]) -> [CpioEntry<'_>; 5] {
    [
        CpioEntry {
            path: "dev",
            mode: DIR_MODE,
            data: &[],
        },
        CpioEntry {
            path: "proc",
            mode: DIR_MODE,
            data: &[],
        },
        CpioEntry {
            path: "sys",
            mode: DIR_MODE,
            data: &[],
        },
        CpioEntry {
            path: "run",
            mode: DIR_MODE,
            data: &[],
        },
        CpioEntry {
            path: "init",
            mode: FILE_MODE,
            data: slave,
        },
    ]
}
