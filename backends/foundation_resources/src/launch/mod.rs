//! Compile the editable launcher icon, splash image, and launch colors for native applications.
use super::{publish, Inputs, Target};
use image::{imageops::FilterType, DynamicImage, ImageFormat, ImageReader};
use serde::Deserialize;
use std::{env, fs, io, path::Path};

mod android;
mod apple;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Theme {
    background: Background,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Background {
    light: String,
    dark: String,
}

fn input_error(path: &Path, error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {error}", path.display()),
    )
}

fn image(path: &Path) -> io::Result<DynamicImage> {
    let mut reader = ImageReader::open(path).map_err(|e| input_error(path, e))?;
    reader.set_format(ImageFormat::Png);
    reader.decode().map_err(|e| input_error(path, e))
}

fn write(dir: &Path, name: &str, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path = dir.join(name);
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| input_error(&path, "Missing output directory"))?,
    )?;
    fs::write(path, contents)
}

/// Compile native launch resources into a directory owned by the build.
/// Android uses its `res` child. Apple uses its `Launch.xcassets` child.
///
/// # Errors
/// Return the source path for an invalid image, color, or theme.
pub fn stage(public: &Path, target: Target, destination: &Path) -> io::Result<()> {
    let settings = public.join("launch-theme.toml");
    let source = fs::read_to_string(&settings).map_err(|e| input_error(&settings, e))?;
    let theme: Theme = toml::from_str(&source).map_err(|e| input_error(&settings, e))?;
    for color in [&theme.background.light, &theme.background.dark] {
        if color.len() != 7
            || !color.starts_with('#')
            || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(input_error(
                &settings,
                format!("Expected #RRGGBB, found {color}"),
            ));
        }
    }
    let icon = public.join("icon.png");
    let picture = image(&icon)?;
    if picture.width() != picture.height()
        || picture.to_rgba8().pixels().any(|pixel| pixel[3] != 255)
    {
        return Err(input_error(
            &icon,
            "The launcher icon must be square and opaque",
        ));
    }
    let splash = public.join("splash.png");
    let splash_picture = image(&splash)?;
    let inputs = tempfile::tempdir()?;
    let name = match target {
        Target::Android => {
            android::write_assets(inputs.path(), &icon, &splash, &theme.background)?;
            "res"
        }
        Target::Apple => {
            apple::write_assets(inputs.path(), &picture, &splash_picture, &theme.background)?;
            "Launch.xcassets"
        }
        Target::Web => {
            return Err(input_error(
                public,
                "Web does not have native launch resources",
            ))
        }
    };
    let mut tree = Inputs::default();
    tree.add(name, &inputs.path().join(name))?;
    publish(&tree, destination)
}

/// Track the editable inputs and compile resources for the current native build.
///
/// # Errors
/// Return an error when a native build specifies an invalid target or input.
pub fn stage_from_cargo(public: &Path) -> io::Result<()> {
    println!("cargo:rerun-if-env-changed=EWE_LAUNCH_DEST");
    println!("cargo:rerun-if-env-changed=EWE_RESOURCE_TARGET");
    let Some(output) = env::var_os("EWE_LAUNCH_DEST") else {
        return Ok(());
    };
    let target = match env::var("EWE_RESOURCE_TARGET").as_deref() {
        Ok("apple") => Target::Apple,
        Ok("android") => Target::Android,
        _ => {
            return Err(input_error(
                public,
                "Set EWE_RESOURCE_TARGET to apple or android",
            ))
        }
    };
    for name in ["launch-theme.toml", "icon.png", "splash.png"] {
        println!("cargo:rerun-if-changed={}", public.join(name).display());
    }
    stage(public, target, Path::new(&output))
}
