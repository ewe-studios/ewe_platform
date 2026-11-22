use super::{BoxedError, ProjectTemplates};
use foundation_packager::{Directorate, PackageConfig, PackageGenerator};
use std::{fs, io, path::Path};

fn identifier(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_alphabetic())
        && word.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn package_identifier(id: &str) -> bool {
    const RESERVED: &[&str] = &[
        "as",
        "break",
        "class",
        "continue",
        "do",
        "else",
        "false",
        "for",
        "fun",
        "if",
        "in",
        "interface",
        "is",
        "null",
        "object",
        "package",
        "return",
        "super",
        "this",
        "throw",
        "true",
        "try",
        "typealias",
        "typeof",
        "val",
        "var",
        "when",
        "while",
    ];
    id.contains('.')
        && id
            .split('.')
            .all(|part| identifier(part) && !RESERVED.contains(&part))
}

pub(super) fn generate(
    template: &str,
    project: &str,
    parent: &Path,
    id: &str,
    host: &str,
) -> Result<(), BoxedError> {
    if !identifier(project) {
        return Err("Project name must start with a letter and contain only letters, numbers, or underscores.".into());
    }
    if !package_identifier(id) || (template == "ios-app" && id.contains('_')) {
        return Err(format!("Invalid application identifier: {id}").into());
    }

    let release: toml::Table =
        include_str!(concat!(env!("OUT_DIR"), "/mobile-release.toml")).parse()?;
    let mut values = serde_json::Map::new();
    for (name, key) in [
        ("VERSION", "version"),
        ("REPOSITORY", "repository"),
        ("MAVEN_GROUP", "maven_group"),
        ("SWIFT_PRODUCT", "swift_product"),
    ] {
        values.insert(
            name.into(),
            release
                .get(key)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("Missing mobile release setting {key}"))?
                .into(),
        );
    }
    for (key, value) in [("PROJECT_NAME", project), ("APP_ID", id), ("HOST", host)] {
        values.insert(key.into(), value.into());
    }
    let dest = parent.join(project);
    fs::create_dir_all(parent)?;
    fs::create_dir(&dest)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", dest.display())))?;
    let build = || -> Result<(), BoxedError> {
        let packager = PackageGenerator::new(Directorate::<ProjectTemplates>::default());
        for variant in ["common", host] {
            packager.create(PackageConfig::new(
                parent.into(),
                values.clone(),
                format!("{template}/{variant}"),
                project.into(),
            ))?;
        }
        if host == "on-device" {
            packager.create(PackageConfig::new(
                dest.clone(),
                values,
                "mobile-rust",
                "rust",
            ))?;
            executable(&dest.join("build-rust.sh"))?;
        }
        if template == "ios-app" {
            let xcode = dest.join("Starter.xcodeproj");
            let schemes = xcode.join("xcshareddata/xcschemes");
            fs::rename(
                schemes.join("Starter.xcscheme"),
                schemes.join(format!("{project}.xcscheme")),
            )?;
            fs::rename(xcode, dest.join(format!("{project}.xcodeproj")))?;
        } else {
            executable(&dest.join("gradlew"))?;
        }
        Ok(())
    };
    if let Err(error) = build() {
        fs::remove_dir_all(&dest)
            .map_err(|cleanup| format!("{error}. Cannot remove {}: {cleanup}", dest.display()))?;
        return Err(error);
    }
    Ok(())
}

fn executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    fs::metadata(path)?;
    Ok(())
}
