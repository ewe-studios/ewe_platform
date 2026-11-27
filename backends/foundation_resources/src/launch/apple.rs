use super::{write, Background, DynamicImage, FilterType, ImageFormat};
use serde_json::{json, Value};
use std::{io, path::Path};

fn color(value: &str) -> io::Result<Value> {
    let component = |start| {
        u8::from_str_radix(&value[start..start + 2], 16)
            .map(|n| format!("{:.6}", f64::from(n) / 255.))
            .map_err(io::Error::other)
    };
    Ok(
        json!({"color-space":"srgb","components":{"red":component(1)?,"green":component(3)?,"blue":component(5)?,"alpha":"1.000"}}),
    )
}

pub(super) fn write_assets(
    root: &Path,
    icon: &DynamicImage,
    splash: &DynamicImage,
    colors: &Background,
) -> io::Result<()> {
    let catalog = root.join("Launch.xcassets");
    let info = json!({"version":1,"author":"xcode"});
    write(
        &catalog,
        "Contents.json",
        serde_json::to_vec(&json!({"info":info}))?,
    )?;
    write(
        &catalog,
        "AppIcon.appiconset/Contents.json",
        serde_json::to_vec(
            &json!({"info":info,"images":[{"idiom":"universal","platform":"ios","size":"1024x1024","filename":"icon.png"}]}),
        )?,
    )?;
    let icon = icon
        .resize_exact(1024, 1024, FilterType::Lanczos3)
        .into_rgb8();
    icon.save_with_format(
        catalog.join("AppIcon.appiconset/icon.png"),
        ImageFormat::Png,
    )
    .map_err(io::Error::other)?;
    let splash_set = catalog.join("EweSplash.imageset");
    std::fs::create_dir_all(&splash_set)?;
    let mut images = Vec::new();
    for scale in [1, 2, 3] {
        let filename = format!("splash-{scale}x.png");
        let side = 144 * scale;
        splash
            .resize(side, side, FilterType::Lanczos3)
            .save_with_format(splash_set.join(&filename), ImageFormat::Png)
            .map_err(io::Error::other)?;
        images.push(json!({"idiom":"universal","scale":format!("{scale}x"),"filename":filename}));
    }
    write(
        &splash_set,
        "Contents.json",
        serde_json::to_vec(&json!({"info":info,"images":images}))?,
    )?;
    write(
        &catalog,
        "EweLaunchBackground.colorset/Contents.json",
        serde_json::to_vec(&json!({
            "info":info,"colors":[{"idiom":"universal","color":color(&colors.light)?},
            {"idiom":"universal","appearances":[{"appearance":"luminosity","value":"dark"}],"color":color(&colors.dark)?}]
        }))?,
    )
}
