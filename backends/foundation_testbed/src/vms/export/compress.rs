//! Post-factum qcow2 compression using Rust-native libraries.
//!
//! Applies gzip or xz (lzma) compression on top of qcow2 files after
//! `qemu-img convert -c`. xz provides the best compression ratio but
//! is slower; gzip is faster but produces larger files.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::vms::config::{Result, TestbedError};

/// Compression algorithm for qcow2 export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    /// No post-factum compression (qcow2-only via qemu-img -c).
    None,
    /// gzip compression (.qcow2.gz) — fast, supports streaming decompress (default).
    #[default]
    Gzip,
    /// xz/lzma compression (.qcow2.xz) — best ratio, requires full file before decompress.
    Xz,
}

impl Compression {
    /// File extension suffix for this compression type.
    pub fn extension(&self) -> &'static str {
        match self {
            Compression::None => "",
            Compression::Gzip => ".gz",
            Compression::Xz => ".xz",
        }
    }
}

/// Compress a qcow2 file with the specified algorithm.
///
/// Reads `src` and writes a compressed archive to `dest`.
/// The dest path should include the compression extension
/// (e.g., `output.qcow2.xz` for xz compression).
pub fn compress_qcow2_with(src: &Path, dest: &Path, algo: Compression) -> Result<()> {
    match algo {
        Compression::None => {
            std::fs::copy(src, dest).map_err(|e| TestbedError::Qcow2Error {
                message: format!("copying qcow2: {e}"),
            })?;
        }
        Compression::Gzip => compress_gzip(src, dest)?,
        Compression::Xz => compress_xz(src, dest)?,
    }
    Ok(())
}

/// Decompress a compressed qcow2 archive to a target path.
///
/// Auto-detects compression from the source file extension.
/// If the source is not compressed, copies it as-is.
pub fn decompress(src: &Path, dest: &Path) -> Result<()> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "gz" => decompress_gzip(src, dest),
        "xz" => decompress_xz(src, dest),
        _ => {
            // Not compressed, copy as-is
            std::fs::copy(src, dest).map_err(|e| TestbedError::Qcow2Error {
                message: format!("copying qcow2: {e}"),
            })?;
            Ok(())
        }
    }
}

/// Check if a path looks like a compressed qcow2 archive.
pub fn is_compressed(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".qcow2.gz") || name.ends_with(".qcow2.xz")
}

/// Resolve the actual image file for a profile, checking for compressed variants.
///
/// Returns the path to the actual qcow2 (decompressing if needed).
/// If the image is compressed, decompresses it to the cache directory.
pub fn ensure_decompressed(image_path: &Path) -> Result<PathBuf> {
    if !is_compressed(image_path) {
        return Ok(image_path.to_path_buf());
    }

    // Decompress to cache
    let cache_dir = crate::vms::config::image_cache_dir();
    std::fs::create_dir_all(&cache_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating image cache dir: {e}"),
    })?;

    // Strip compression extension to get the qcow2 name
    let qcow2_name = image_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.trim_end_matches(".gz")
                .trim_end_matches(".xz")
                .to_string()
        })
        .unwrap_or_else(|| "image.qcow2".to_string());

    let dest = cache_dir.join(&qcow2_name);

    // Already decompressed
    if dest.exists() {
        let meta = std::fs::metadata(&dest).map_err(|e| TestbedError::Qcow2Error {
            message: format!("stat decompressed image: {e}"),
        })?;
        if meta.len() > 0 {
            return Ok(dest);
        }
    }

    let temp_dest = dest.with_extension("decompressing");
    decompress(image_path, &temp_dest)?;
    std::fs::rename(&temp_dest, &dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("moving decompressed image: {e}"),
    })?;

    Ok(dest)
}

// ── Gzip compression (Rust-native via flate2) ────────────────────────────────

pub fn compress_gzip(src: &Path, dest: &Path) -> Result<()> {
    let src_file = File::open(src).map_err(|e| TestbedError::Qcow2Error {
        message: format!("opening source: {e}"),
    })?;
    let dest_file = File::create(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating destination: {e}"),
    })?;

    let mut reader = BufReader::new(src_file);
    let mut encoder = flate2::write::GzEncoder::new(
        BufWriter::new(dest_file),
        flate2::Compression::best(),
    );

    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf).map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading source: {e}"),
        })?;
        if n == 0 { break; }
        encoder.write_all(&buf[..n]).map_err(|e| TestbedError::Qcow2Error {
            message: format!("writing compressed data: {e}"),
        })?;
    }

    encoder.finish().map_err(|e| TestbedError::Qcow2Error {
        message: format!("finalizing gzip: {e}"),
    })?;

    Ok(())
}

pub fn decompress_gzip(src: &Path, dest: &Path) -> Result<()> {
    let src_file = File::open(src).map_err(|e| TestbedError::Qcow2Error {
        message: format!("opening source: {e}"),
    })?;
    let dest_file = File::create(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating destination: {e}"),
    })?;

    let mut decoder = flate2::read::GzDecoder::new(BufReader::new(src_file));
    let mut writer = BufWriter::new(dest_file);

    let mut buf = [0u8; 65536];
    loop {
        let n = decoder.read(&mut buf).map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading compressed data: {e}"),
        })?;
        if n == 0 { break; }
        writer.write_all(&buf[..n]).map_err(|e| TestbedError::Qcow2Error {
            message: format!("writing decompressed data: {e}"),
        })?;
    }

    writer.flush().map_err(|e| TestbedError::Qcow2Error {
        message: format!("flushing output: {e}"),
    })?;

    Ok(())
}

// ── XZ compression (Rust-native via xz2) ─────────────────────────────────────

pub fn compress_xz(src: &Path, dest: &Path) -> Result<()> {
    let src_file = File::open(src).map_err(|e| TestbedError::Qcow2Error {
        message: format!("opening source: {e}"),
    })?;
    let dest_file = File::create(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating destination: {e}"),
    })?;

    let mut reader = BufReader::new(src_file);
    // xz2 level 9 = max compression (equivalent to xz -9)
    let mut encoder = xz2::write::XzEncoder::new(
        BufWriter::new(dest_file),
        9,
    );

    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf).map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading source: {e}"),
        })?;
        if n == 0 { break; }
        encoder.write_all(&buf[..n]).map_err(|e| TestbedError::Qcow2Error {
            message: format!("writing compressed data: {e}"),
        })?;
    }

    encoder.finish().map_err(|e| TestbedError::Qcow2Error {
        message: format!("finalizing xz: {e}"),
    })?;

    Ok(())
}

pub fn decompress_xz(src: &Path, dest: &Path) -> Result<()> {
    let src_file = File::open(src).map_err(|e| TestbedError::Qcow2Error {
        message: format!("opening source: {e}"),
    })?;
    let dest_file = File::create(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating destination: {e}"),
    })?;

    let mut decoder = xz2::read::XzDecoder::new(BufReader::new(src_file));
    let mut writer = BufWriter::new(dest_file);

    let mut buf = [0u8; 65536];
    loop {
        let n = decoder.read(&mut buf).map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading compressed data: {e}"),
        })?;
        if n == 0 { break; }
        writer.write_all(&buf[..n]).map_err(|e| TestbedError::Qcow2Error {
            message: format!("writing decompressed data: {e}"),
        })?;
    }

    writer.flush().map_err(|e| TestbedError::Qcow2Error {
        message: format!("flushing output: {e}"),
    })?;

    Ok(())
}

/// Apply system-level compression as a fallback (if Rust-native fails).
/// Uses `xz` or `gzip` from the system PATH.
#[allow(dead_code)]
pub fn compress_system(src: &Path, dest: &Path, algo: Compression) -> Result<()> {
    let (cmd, args) = match algo {
        Compression::Xz => ("xz", vec!["-9", "-c", src.to_str().unwrap()]),
        Compression::Gzip => ("gzip", vec!["-9", "-c", src.to_str().unwrap()]),
        Compression::None => {
            std::fs::copy(src, dest).map_err(|e| TestbedError::Qcow2Error {
                message: format!("copying qcow2: {e}"),
            })?;
            return Ok(());
        }
    };

    let out_file = File::create(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating destination: {e}"),
    })?;

    let status = Command::new(cmd)
        .args(&args)
        .stdout(out_file)
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning {cmd}: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("{cmd} compression failed (exit {status})"),
        });
    }

    Ok(())
}

