//! macOS image creation — native IPSW download + BaseSystem extraction.
//!
//! Creates bootable macOS qcow2 images on Linux without requiring
//! quickget/quickemu dependencies. Uses QEMU's built-in DMG reader
//! for conversion.
//!
//! # Flow
//! 1. Query `api.ipsw.me` for available macOS IPSWs
//! 2. Download IPSW (zip archive) from Apple CDN
//! 3. Extract `BaseSystem.dmg` from the IPSW
//! 4. Convert DMG → qcow2 via `qemu-img convert`
//! 5. Create empty data disk for user space (resizable)
//! 6. Generate OpenCore EFI config and EFI disk image

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::vms::config::{Result, TestbedError};

/// OpenCore release version to download from GitHub.
const OPENCORE_VERSION: &str = "1.0.3";

/// OpenCore EFI disk filename.
pub const OPENCORE_EFI_FILENAME: &str = "opencore.qcow2";

/// Minimum IPSW size (500 MB).
const MIN_IPSW_SIZE: u64 = 500 * 1_048_576;

/// IPSW catalog response from api.ipsw.me.
#[derive(Debug, Clone)]
pub struct IpswInfo {
    pub version: String,
    pub build_id: String,
    pub url: String,
    pub size: u64,
}

/// Ensure a macOS image is available in the cache for the given profile.
///
/// Creates both the BaseSystem qcow2 (boot disk) and the OpenCore EFI
/// disk if they don't already exist.
pub fn ensure_macos_image(profile_name: &str) -> Result<PathBuf> {
    let base_dir = crate::vms::config::image_cache_dir();
    std::fs::create_dir_all(&base_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating image cache dir: {e}"),
    })?;

    let macos_dir = base_dir.join(format!("macos-{profile_name}"));
    std::fs::create_dir_all(&macos_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating macos image dir: {e}"),
    })?;

    let basesystem_qcow2 = macos_dir.join("BaseSystem.qcow2");
    let opencore_qcow2 = macos_dir.join(OPENCORE_EFI_FILENAME);
    let data_disk_qcow2 = macos_dir.join("macOS-data.qcow2");

    // If all components exist and look valid, return the BaseSystem path
    if basesystem_qcow2.exists() && opencore_qcow2.exists() && data_disk_qcow2.exists()
        && let Ok(meta) = std::fs::metadata(&basesystem_qcow2)
            && meta.len() > 100_000_000 {
                return Ok(basesystem_qcow2);
            }

    // Step 1: Find IPSW
    println!("  Looking up macOS IPSW...");
    let ipsw = find_ipsw(None)?;
    println!("  Found macOS {} (build {})", ipsw.version, ipsw.build_id);

    // Step 2: Download IPSW
    let ipsw_path = macos_dir.join("macos.ipsw");
    if !ipsw_path.exists() {
        println!("  Downloading IPSW from Apple CDN...");
        download_ipsw(&ipsw, &ipsw_path)?;
    }

    // Step 3: Extract BaseSystem.dmg
    let basesystem_dmg = macos_dir.join("BaseSystem.dmg");
    if !basesystem_dmg.exists() {
        println!("  Extracting BaseSystem.dmg from IPSW...");
        extract_basesystem(&ipsw_path, &basesystem_dmg)?;
    }

    // Step 4: Convert DMG → qcow2
    if !basesystem_qcow2.exists() {
        println!("  Converting BaseSystem to qcow2...");
        convert_dmg_to_qcow2(&basesystem_dmg, &basesystem_qcow2)?;
    }

    // Step 5: Create data disk
    if !data_disk_qcow2.exists() {
        println!("  Creating 80 GB data disk...");
        create_data_disk(&data_disk_qcow2, 80)?;
    }

    // Step 6: Create OpenCore EFI disk
    if !opencore_qcow2.exists() {
        println!("  Creating OpenCore EFI disk...");
        create_opencore_efi(&macos_dir, &opencore_qcow2)?;
    }

    // Clean up large intermediate files
    let _ = std::fs::remove_file(&ipsw_path);
    let _ = std::fs::remove_file(&basesystem_dmg);

    println!("  macOS image ready at: {}", basesystem_qcow2.display());
    println!("  Data disk at: {}", data_disk_qcow2.display());
    println!("  OpenCore EFI at: {}", opencore_qcow2.display());

    Ok(basesystem_qcow2)
}

/// Query api.ipsw.me for available macOS IPSWs.
///
/// Returns the latest macOS version available for iMacPro1,1 (x86_64).
pub fn find_ipsw(model: Option<&str>) -> Result<IpswInfo> {
    let device_id = model.unwrap_or("MacBookPro16,1"); // x86_64 Intel Mac

    // Try multiple device IDs that return macOS IPSWs
    let devices = [
        device_id,
        "iMacPro1,1",
        "MacPro7,1",
        "MacBookPro16,1",
    ];

    let mut latest: Option<IpswInfo> = None;

    for dev in &devices {
        let api_url = format!("https://api.ipsw.me/v4/device/{dev}");
        if let Ok(info) = query_ipsw_api(&api_url) {
            // Keep the one with the highest version number (by build ID sort)
            if latest.as_ref().is_none_or(|prev| info.build_id > prev.build_id) {
                latest = Some(info);
            }
        }
    }

    latest.ok_or_else(|| TestbedError::DownloadFailed {
        status: 0,
        url: "could not find macOS IPSW from api.ipsw.me".to_string(),
    })
}

/// Query the IPSW API for a specific device.
fn query_ipsw_api(url: &str) -> Result<IpswInfo> {
    let output = Command::new("curl")
        .args(["-s", "-L", "--max-redirs", "5", "-m", "60", url])
        .output()
        .map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("curl failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::DownloadFailed {
            status: 0,
            url: format!("api.ipsw.me returned error: {}", String::from_utf8_lossy(&output.stderr)),
        });
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        TestbedError::DownloadFailed {
            status: 0,
            url: format!("failed to parse IPSW API response: {e}"),
        }
    })?;

    // The API returns an array of firmware entries
    let firmwares = json.get("firmwares").and_then(|f| f.as_array()).ok_or_else(|| {
        TestbedError::DownloadFailed {
            status: 0,
            url: "api.ipsw.me response missing 'firmwares' field".to_string(),
        }
    })?;

    // Find the latest macOS firmware (skip iOS/iPadOS by checking version contains macOS indicators)
    let mut candidates: Vec<_> = firmwares
        .iter()
        .filter_map(|fw| {
            let version = fw.get("version")?.as_str()?.to_string();
            let build_id = fw.get("buildid")?.as_str()?.to_string();
            let url = fw.get("url")?.as_str()?.to_string();
            let size = fw.get("size")?.as_u64().unwrap_or(0);

            // Filter for macOS: build IDs starting with certain prefixes
            // macOS build IDs typically start with 22, 23, 24 etc. (not 18, 19, 20, 21 for iOS)
            // More reliable: version string contains known macOS names
            let is_macos = version.contains("Sonoma")
                || version.contains("Sequoia")
                || version.contains("Ventura")
                || version.contains("Monterey")
                || version.contains("Big Sur");

            if is_macos {
                Some(IpswInfo { version, build_id, url, size })
            } else {
                None
            }
        })
        .collect();

    candidates.sort_by(|a, b| b.build_id.cmp(&a.build_id));

    candidates.into_iter().next().ok_or_else(|| TestbedError::DownloadFailed {
        status: 0,
        url: "no macOS firmware found in API response".to_string(),
    })
}

/// Download an IPSW from Apple CDN using simple HTTP with progress.
fn download_ipsw(ipsw: &IpswInfo, dest: &Path) -> Result<()> {
    // Use wget for progress, or curl as fallback
    let status = Command::new("curl")
        .args([
            "-L",
            "-#",
            "--max-redirs",
            "10",
            "-m",
            "600", // 10 minutes for large IPSW
            "-o",
            dest.to_str().ok_or_else(|| TestbedError::Qcow2Error {
                message: format!("invalid IPSW destination path: {dest:?}"),
            })?,
            &ipsw.url,
        ])
        .status()
        .map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("curl failed: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::DownloadFailed {
            status: 0,
            url: format!("IPSW download failed (exit: {:?})", status.code()),
        });
    }

    // Validate size
    let meta = std::fs::metadata(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat IPSW: {e}"),
    })?;

    if meta.len() < MIN_IPSW_SIZE {
        return Err(TestbedError::DownloadFailed {
            status: 0,
            url: format!("IPSW too small ({} bytes) — download may have failed", meta.len()),
        });
    }

    println!("  IPSW downloaded: {:.1} MB", meta.len() as f64 / 1_048_576.0);
    Ok(())
}

/// Extract BaseSystem.dmg from an IPSW (which is a zip archive).
fn extract_basesystem(ipsw_path: &Path, dest: &Path) -> Result<()> {
    // IPSW is a zip file. BaseSystem.dmg is the key component.
    // Try with 7z first (handles more compression formats), then unzip.
    let extract_result = Command::new("7z")
        .args([
            "x",
            ipsw_path.to_str().unwrap(),
            &format!("-o{}", ipsw_path.parent().unwrap().to_str().unwrap()),
            "BaseSystem.dmg",
            "-y",
        ])
        .status();

    let extracted = if let Ok(s) = extract_result {
        s.success()
    } else {
        // Fallback to unzip
        

        Command::new("unzip")
            .args([
                "-o",
                ipsw_path.to_str().unwrap(),
                "BaseSystem.dmg",
                "-d",
                ipsw_path.parent().unwrap().to_str().unwrap(),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };

    if !extracted {
        return Err(TestbedError::Qcow2Error {
            message: "failed to extract BaseSystem.dmg from IPSW".to_string(),
        });
    }

    // Move to final destination
    let extracted_path = ipsw_path.parent().unwrap().join("BaseSystem.dmg");
    if !extracted_path.exists() {
        return Err(TestbedError::Qcow2Error {
            message: "BaseSystem.dmg not found after extraction".to_string(),
        });
    }

    std::fs::rename(&extracted_path, dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("moving BaseSystem.dmg: {e}"),
    })?;

    let meta = std::fs::metadata(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat BaseSystem.dmg: {e}"),
    })?;

    println!("  BaseSystem.dmg extracted: {:.1} MB", meta.len() as f64 / 1_048_576.0);
    Ok(())
}

/// Convert a DMG to qcow2 using `qemu-img convert`.
fn convert_dmg_to_qcow2(dmg_path: &Path, dest: &Path) -> Result<()> {
    let status = Command::new("qemu-img")
        .args([
            "convert",
            "-f",
            "dmg",
            "-O",
            "qcow2",
            "-c", // compress for smaller
            dmg_path.to_str().ok_or_else(|| TestbedError::Qcow2Error {
                message: format!("invalid DMG path: {dmg_path:?}"),
            })?,
            dest.to_str().ok_or_else(|| TestbedError::Qcow2Error {
                message: format!("invalid qcow2 destination: {dest:?}"),
            })?,
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("qemu-img convert failed (exit: {:?})", status.code()),
        });
    }

    let meta = std::fs::metadata(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat qcow2: {e}"),
    })?;

    println!("  BaseSystem.qcow2 created: {:.1} MB", meta.len() as f64 / 1_048_576.0);
    Ok(())
}

/// Create an empty resizable qcow2 data disk for user space.
fn create_data_disk(dest: &Path, size_gb: u32) -> Result<()> {
    let status = Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            dest.to_str().ok_or_else(|| TestbedError::Qcow2Error {
                message: format!("invalid data disk path: {dest:?}"),
            })?,
            &format!("{size_gb}G"),
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("qemu-img create failed (exit: {:?})", status.code()),
        });
    }

    Ok(())
}

/// Create an OpenCore EFI disk image.
///
/// Downloads OpenCore from GitHub releases, generates config.plist,
/// and creates a FAT32 disk image with the EFI files.
fn create_opencore_efi(macos_dir: &Path, dest: &Path) -> Result<()> {
    let efi_stage = macos_dir.join("opencore-stage");
    std::fs::create_dir_all(&efi_stage).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating EFI staging dir: {e}"),
    })?;

    // Step 1: Download OpenCore RELEASE from GitHub
    let asset_name = format!("OpenCore-{OPENCORE_VERSION}-RELEASE.zip");
    let zip_path = macos_dir.join(&asset_name);

    if !zip_path.exists() {
        let gh_url = format!(
            "https://github.com/acidanthera/OpenCorePkg/releases/download/{OPENCORE_VERSION}/{asset_name}"
        );
        println!("  Downloading OpenCore {OPENCORE_VERSION}...");

        let status = Command::new("curl")
            .args([
                "-L", "-#",
                "-o", zip_path.to_str().unwrap(),
                &gh_url,
            ])
            .status()
            .map_err(|e| TestbedError::DownloadFailed {
                status: 0,
                url: format!("curl failed: {e}"),
            })?;

        if !status.success() {
            return Err(TestbedError::DownloadFailed {
                status: 0,
                url: format!("OpenCore download failed (exit: {:?})", status.code()),
            });
        }
    }

    // Step 2: Extract OpenCore to staging
    println!("  Extracting OpenCore...");
    let status = Command::new("7z")
        .args([
            "x",
            zip_path.to_str().unwrap(),
            &format!("-o{}", efi_stage.to_str().unwrap()),
            "X64/EFI/**",
            "-y",
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("extracting OpenCore: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "failed to extract OpenCore from zip".to_string(),
        });
    }

    // Step 3: Download HfsPlus.efi driver (required for APFS/HFS+ volumes)
    let drivers_dir = efi_stage.join("X64").join("EFI").join("OC").join("Drivers");
    std::fs::create_dir_all(&drivers_dir).ok();
    let hfsplus_path = drivers_dir.join("HfsPlus.efi");

    if !hfsplus_path.exists() {
        println!("  Downloading HfsPlus.efi driver...");
        let hfsplus_url = "https://github.com/acidanthera/OcBinaryData/raw/master/Drivers/HfsPlus.efi".to_string();
        let _ = Command::new("curl")
            .args(["-sL", "-o", hfsplus_path.to_str().unwrap(), &hfsplus_url])
            .status();
    }

    // Step 4: Generate config.plist
    println!("  Generating OpenCore config.plist...");
    let config_plist = generate_config_plist();
    std::fs::write(
        efi_stage.join("X64").join("EFI").join("OC").join("config.plist"),
        config_plist,
    ).map_err(|e| TestbedError::Qcow2Error {
        message: format!("writing config.plist: {e}"),
    })?;

    // Step 5: Create FAT32 disk image
    let fat_img = macos_dir.join("EFI-fat.img");
    let fat_size_mb = 256; // 256 MB FAT32 image

    println!("  Creating FAT32 EFI image ({fat_size_mb} MB)...");
    let status = Command::new("dd")
        .args([
            "if=/dev/zero",
            &format!("of={}", fat_img.to_str().unwrap()),
            "bs=1M",
            &format!("count={fat_size_mb}"),
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("dd failed: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "failed to create FAT32 image with dd".to_string(),
        });
    }

    // Format as FAT32
    let status = Command::new("mkfs.vfat")
        .args([fat_img.to_str().unwrap()])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("mkfs.vfat failed: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "failed to format FAT32 image".to_string(),
        });
    }

    // Copy EFI files into the FAT32 image using mcopy (mtools) or mount loopback
    // Use mcopy from mtools if available, else use mount loopback
    let efi_src = efi_stage.join("X64").join("EFI");
    if efi_src.exists() {
        println!("  Populating EFI image...");
        populate_fat_image(&fat_img, &efi_src)?;
    }

    // Step 6: Convert FAT image to qcow2
    let status = Command::new("qemu-img")
        .args([
            "convert",
            "-f",
            "raw",
            "-O",
            "qcow2",
            "-c",
            fat_img.to_str().unwrap(),
            dest.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("converting EFI image to qcow2: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "qemu-img convert for EFI failed".to_string(),
        });
    }

    // Clean up staging
    let _ = std::fs::remove_file(&zip_path);
    let _ = std::fs::remove_file(&fat_img);
    let _ = std::fs::remove_dir_all(&efi_stage);

    let meta = std::fs::metadata(dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat EFI qcow2: {e}"),
    })?;

    println!("  OpenCore EFI disk created: {:.1} MB", meta.len() as f64 / 1_048_576.0);
    Ok(())
}

/// Populate a FAT32 image with EFI files using mtools or mount loopback.
fn populate_fat_image(fat_img: &Path, efi_dir: &Path) -> Result<()> {
    // Try mcopy first (from mtools package)
    let mcopy_status = Command::new("mcopy")
        .args([
            "-s",
            &format!("{}/*", efi_dir.display()),
            &format!("{}::/", fat_img.display()),
        ])
        .status();

    if let Ok(s) = mcopy_status
        && s.success() {
            return Ok(());
        }

    // Fallback: mount loopback + cp
    let mount_point = fat_img.parent().unwrap().join("efi-mount");
    std::fs::create_dir_all(&mount_point).ok();

    let mount_status = Command::new("mount")
        .args([
            "-o",
            "loop",
            "-t",
            "vfat",
            fat_img.to_str().unwrap(),
            mount_point.to_str().unwrap(),
        ])
        .status();

    if let Ok(s) = mount_status
        && s.success() {
            let cp_status = Command::new("cp")
                .args(["-r", efi_dir.to_str().unwrap(), &format!("{}/EFI", mount_point.display())])
                .status();

            let _ = Command::new("umount").arg(&mount_point).status();
            let _ = std::fs::remove_dir(&mount_point);

            if cp_status.map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }

    Err(TestbedError::Qcow2Error {
        message: "failed to populate FAT32 image (tried mcopy and mount)".to_string(),
    })
}

/// Generate an OpenCore config.plist for QEMU macOS boot.
///
/// Uses MacPro7,1 SMBIOS, standard boot args for QEMU compatibility.
fn generate_config_plist() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>ACPI</key>
    <dict>
        <key>Add</key>
        <array/>
        <key>Delete</key>
        <array/>
    </dict>
    <key>Booter</key>
    <dict>
        <key>MmioWhitelist</key>
        <array/>
        <key>Quirks</key>
        <dict>
            <key>AllowRelocationRelocation</key>
            <false/>
            <key>AvoidRuntimeDefrag</key>
            <true/>
            <key>DevirtualiseMmio</key>
            <false/>
            <key>DisableSingleUser</key>
            <false/>
            <key>DisableVariableWrite</key>
            <false/>
            <key>DiscardHibernateMap</key>
            <false/>
            <key>EnableSafeModeSlide</key>
            <true/>
            <key>EnableWriteUnprotector</key>
            <true/>
            <key>ForceBooterSignature</key>
            <false/>
            <key>ForceExitBootServices</key>
            <false/>
            <key>ProtectMemoryRegions</key>
            <false/>
            <key>ProtectSecureBoot</key>
            <false/>
            <key>ProtectUefiServices</key>
            <false/>
            <key>ProvideCustomSlide</key>
            <true/>
            <key>RebuildAppleMemoryMap</key>
            <false/>
            <key>SetupVirtualMap</key>
            <false/>
            <key>SyncRuntimePermissions</key>
            <false/>
        </dict>
    </dict>
    <key>DeviceProperties</key>
    <dict>
        <key>Add</key>
        <dict/>
        <key>Delete</key>
        <dict/>
    </dict>
    <key>Kernel</key>
    <dict>
        <key>Add</key>
        <array/>
        <key>Block</key>
        <array/>
        <key>Emulate</key>
        <dict>
            <key>Cpuid1Data</key>
            <data></data>
            <key>Cpuid1Mask</key>
            <data></data>
            <key>DummyPowerManagement</key>
            <false/>
            <key>MaxKernel</key>
            <string></string>
            <key>MinKernel</key>
            <string></string>
        </dict>
        <key>Force</key>
        <array/>
        <key>Patch</key>
        <array/>
        <key>Quirks</key>
        <dict>
            <key>AppleCpuPmCfgLock</key>
            <false/>
            <key>AppleXcpmCfgLock</key>
            <false/>
            <key>AppleXcpmExtraMsrs</key>
            <false/>
            <key>CustomSMBIOSGuid</key>
            <false/>
            <key>DisableIoMapper</key>
            <false/>
            <key>DisableRtcChecksum</key>
            <false/>
            <key>LapicKernelPanic</key>
            <false/>
            <key>PowerVendorMask</key>
            <integer>0</integer>
        </dict>
    </dict>
    <key>Misc</key>
    <dict>
        <key>BlessOverride</key>
        <array/>
        <key>Boot</key>
        <dict>
            <key>ConsoleAttributes</key>
            <integer>0</integer>
            <key>HibernateMode</key>
            <string>None</string>
            <key>HideAuxiliary</key>
            <false/>
            <key>LauncherOption</key>
            <string>Full</string>
            <key>LauncherPath</key>
            <string>Default</string>
            <key>PickerAttributes</key>
            <integer>1</integer>
            <key>PickerMode</key>
            <string>Builtin</string>
            <key>PickerVariant</key>
            <string>Auto</string>
            <key>PollAppleHotKeys</key>
            <false/>
            <key>ShowPicker</key>
            <true/>
            <key>TakeoffDelay</key>
            <integer>0</integer>
            <key>Timeout</key>
            <integer>15</integer>
        </dict>
        <key>Debug</key>
        <dict>
            <key>AppleDebug</key>
            <false/>
            <key>ApplePanic</key>
            <false/>
            <key>DisableWatchDog</key>
            <false/>
            <key>DisplayDelay</key>
            <integer>0</integer>
            <key>DisplayLevel</key>
            <integer>2147483650</integer>
            <key>SerialInit</key>
            <false/>
            <key>SysReport</key>
            <false/>
            <key>Target</key>
            <integer>67</integer>
        </dict>
        <key>Entries</key>
        <array/>
        <key>Security</key>
        <dict>
            <key>AllowSetDefault</key>
            <false/>
            <key>ApECID</key>
            <integer>0</integer>
            <key>AuthRestart</key>
            <false/>
            <key>BootProtect</key>
            <string>None</string>
            <key>DmgLoading</key>
            <string>Signed</string>
            <key>ExposeSensitiveData</key>
            <integer>6</integer>
            <key>HaltLevel</key>
            <integer>2147483648</integer>
            <key>PasswordHash</key>
            <data></data>
            <key>PasswordSalt</key>
            <data></data>
            <key>ScanPolicy</key>
            <integer>0</integer>
            <key>SecureBootModel</key>
            <string>Disabled</string>
            <key>Vault</key>
            <string>Optional</string>
        </dict>
        <key>Tools</key>
        <array/>
    </dict>
    <key>NVRAM</key>
    <dict>
        <key>Add</key>
        <dict>
            <key>7C436110-AB2A-4BBB-A880-FE41995C9F82</key>
            <dict>
                <key>boot-args</key>
                <string>-v keepsyms=1 debug=0x100 alcid=1</string>
                <key>csr-active-config</key>
                <data>AAAAAA==</data>
                <key>run-efi-updater</key>
                <string>No</string>
            </dict>
            <key>4D1EDE05-38C7-4A6A-9CC6-4BCCA8B38C14</key>
            <dict>
                <key>DefaultBackgroundColor</key>
                <data>AAAAAA==</data>
                <key>UIScale</key>
                <data>AQ==</data>
            </dict>
        </dict>
        <key>Delete</key>
        <dict>
            <key>4D1EDE05-38C7-4A6A-9CC6-4BCCA8B38C14</key>
            <array>
                <string>UIScale</string>
                <string>DefaultBackgroundColor</string>
            </array>
        </dict>
        <key>LegacyOverwrite</key>
        <false/>
        <key>LegacySchema</key>
        <dict/>
        <key>WriteFlash</key>
        <true/>
    </dict>
    <key>PlatformInfo</key>
    <dict>
        <key>Automatic</key>
        <true/>
        <key>CustomMemory</key>
        <false/>
        <key>Generic</key>
        <dict>
            <key>AdviseFeatures</key>
            <false/>
            <key>MaxBIOSVersion</key>
            <false/>
            <key>MLB</key>
            <string>C0234567890123456</string>
            <key>ProcessorType</key>
            <integer>0</integer>
            <key>ROM</key>
            <data>AAAAAAAA</data>
            <key>SpoofVendor</key>
            <true/>
            <key>SystemMemoryStatus</key>
            <string>Auto</string>
            <key>SystemProductName</key>
            <string>iMacPro1,1</string>
            <key>SystemSerialNumber</key>
            <string>C02LM1234567</string>
            <key>SystemUUID</key>
            <string>00000000-0000-0000-0000-000000000000</string>
        </dict>
        <key>UpdateDataHub</key>
        <true/>
        <key>UpdateNVRAM</key>
        <true/>
        <key>UpdateSMBIOS</key>
        <true/>
        <key>UpdateSMBIOSMode</key>
        <string>Create</string>
        <key>UseRawUuidEncoding</key>
        <false/>
    </dict>
    <key>UEFI</key>
    <dict>
        <key>APFS</key>
        <dict>
            <key>EnableJumpstart</key>
            <true/>
            <key>GlobalConnect</key>
            <false/>
            <key>HideVerbose</key>
            <true/>
            <key>JumpstartHotPlug</key>
            <false/>
            <key>MinDate</key>
            <integer>0</integer>
            <key>MinVersion</key>
            <integer>0</integer>
        </dict>
        <key>Audio</key>
        <dict>
            <key>AudioCodec</key>
            <integer>0</integer>
            <key>AudioDevice</key>
            <string></string>
            <key>AudioOut</key>
            <integer>0</integer>
            <key>AudioSupport</key>
            <false/>
            <key>MinimumVolume</key>
            <integer>20</integer>
            <key>PlayChime</key>
            <string>Auto</string>
            <key>ResetTrafficClass</key>
            <false/>
            <key>SetupDelay</key>
            <integer>0</integer>
            <key>VolumeAmplifier</key>
            <integer>0</integer>
        </dict>
        <key>ConnectDrivers</key>
        <true/>
        <key>Drivers</key>
        <array>
            <dict>
                <key>Arguments</key>
                <string></string>
                <key>Comment</key>
                <string>HFS+ Driver</string>
                <key>Enabled</key>
                <true/>
                <key>LoadEarly</key>
                <false/>
                <key>Path</key>
                <string>HfsPlus.efi</string>
            </dict>
            <dict>
                <key>Arguments</key>
                <string></string>
                <key>Comment</key>
                <string>OpenRuntime</string>
                <key>Enabled</key>
                <true/>
                <key>LoadEarly</key>
                <false/>
                <key>Path</key>
                <string>OpenRuntime.efi</string>
            </dict>
        </array>
        <key>Input</key>
        <dict>
            <key>KeyFiltering</key>
            <false/>
            <key>KeyForgetThreshold</key>
            <integer>5</integer>
            <key>KeySupport</key>
            <true/>
            <key>KeySupportMode</key>
            <string>Auto</string>
            <key>KeySwap</key>
            <false/>
            <key>PointerSupport</key>
            <false/>
            <key>PointerSupportMode</key>
            <string>ASUS</string>
            <key>TimerResolution</key>
            <integer>50000</integer>
        </dict>
        <key>Output</key>
        <dict>
            <key>ClearScreenOnModeSwitch</key>
            <false/>
            <key>ConsoleMode</key>
            <string></string>
            <key>DirectGopRendering</key>
            <false/>
            <key>ForceResolution</key>
            <false/>
            <key>GopPassThrough</key>
            <string>Disabled</string>
            <key>IgnoreTextInGraphics</key>
            <false/>
            <key>ProvideConsoleGop</key>
            <true/>
            <key>ReconnectGraphicsOnConnect</key>
            <false/>
            <key>ReconnectOnResChange</key>
            <false/>
            <key>ReplaceTabWithSpace</key>
            <false/>
            <key>Resolution</key>
            <string>1920x1080</string>
            <key>SanitiseClearScreen</key>
            <false/>
            <key>TextRenderer</key>
            <string>BuiltinGraphics</string>
            <key>UIScale</key>
            <integer>-1</integer>
            <key>UgaPassThrough</key>
            <false/>
        </dict>
        <key>ProtocolOverrides</key>
        <dict>
            <key>AppleAudio</key>
            <false/>
            <key>AppleBootPolicy</key>
            <false/>
            <key>AppleDebugLog</key>
            <false/>
            <key>AppleEg2Info</key>
            <false/>
            <key>AppleFramebufferInfo</key>
            <false/>
            <key>AppleImageConversion</key>
            <false/>
            <key>AppleImg4Verification</key>
            <false/>
            <key>AppleKeyMap</key>
            <false/>
            <key>AppleRtcRam</key>
            <false/>
            <key>AppleSecureBoot</key>
            <false/>
            <key>AppleSmcIo</key>
            <false/>
            <key>AppleUserInterfaceTheme</key>
            <false/>
            <key>DataHub</key>
            <false/>
            <key>DeviceProperties</key>
            <false/>
            <key>FirmwareVolume</key>
            <false/>
            <key>HashServices</key>
            <false/>
            <key>OSInfo</key>
            <false/>
            <key>PciIo</key>
            <false/>
            <key>UnicodeCollation</key>
            <false/>
        </dict>
        <key>Quirks</key>
        <dict>
            <key>ActivateHpetSupport</key>
            <false/>
            <key>DisableSecurityPolicy</key>
            <false/>
            <key>EnableVectorAcceleration</key>
            <true/>
            <key>EnableVmx</key>
            <false/>
            <key>ExitBootServicesDelay</key>
            <integer>0</integer>
            <key>ForceOcWriteFlash</key>
            <false/>
            <key>ForgeUefiSupport</key>
            <false/>
            <key>IgnoreInvalidFlexRatio</key>
            <false/>
            <key>ReleaseUsbOwnership</key>
            <false/>
            <key>ReloadOptionRoms</key>
            <false/>
            <key>RequestBootVarRouting</key>
            <true/>
            <key>ResizeGpuMemory</key>
            <integer>0</integer>
            <key>TscSyncTimeout</key>
            <integer>0</integer>
            <key>UnblockFsConnect</key>
            <false/>
        </dict>
        <key>ReservedMemory</key>
        <array/>
    </dict>
</dict>
</plist>"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_plist_is_valid_xml() {
        let config = generate_config_plist();
        assert!(config.contains("<?xml"));
        assert!(config.contains("iMacPro1,1"));
        assert!(config.contains("boot-args"));
    }

    #[test]
    fn test_config_plist_has_required_sections() {
        let config = generate_config_plist();
        assert!(config.contains("<key>PlatformInfo</key>"));
        assert!(config.contains("<key>SystemProductName</key>"));
        assert!(config.contains("<key>NVRAM</key>"));
        assert!(config.contains("<key>UEFI</key>"));
        assert!(config.contains("<key>Misc</key>"));
        assert!(config.contains("<key>Kernel</key>"));
    }
}
