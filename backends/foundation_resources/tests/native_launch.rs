#![cfg(feature = "mobile-launch")]

use foundation_resources::build::{launch, Target};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use std::{fs, path::Path};

fn inputs(public: &Path, tint: u8, light: &str, dark: &str) {
    fs::create_dir_all(public).unwrap();
    RgbImage::from_pixel(16, 16, Rgb([tint, 0, 0]))
        .save(public.join("icon.png"))
        .unwrap();
    RgbaImage::from_pixel(8, 12, Rgba([0, tint, 0, 128]))
        .save(public.join("splash.png"))
        .unwrap();
    fs::write(
        public.join("launch-theme.toml"),
        format!("[background]\nlight='{light}'\ndark='{dark}'\n"),
    )
    .unwrap();
}

#[test]
fn the_same_three_source_paths_update_both_native_resource_sets() {
    let root = tempfile::tempdir().unwrap();
    let public = root.path().join("public");
    let android = root.path().join("android");
    let apple = root.path().join("apple");
    for (tint, light, dark) in [(16, "#F8FAFC", "#101820"), (225, "#FF1100", "#00CC22")] {
        inputs(&public, tint, light, dark);
        launch::stage(&public, Target::Android, &android).unwrap();
        launch::stage(&public, Target::Apple, &apple).unwrap();
        let android = android.join("res");
        for (dir, color) in [("values", light), ("values-night", dark)] {
            assert!(
                fs::read_to_string(android.join(dir).join("ewe_launch_colors.xml"))
                    .unwrap()
                    .contains(color)
            );
        }
        let icon = android.join("drawable-nodpi/ewe_launch_icon.png");
        let splash = android.join("drawable-nodpi/ewe_launch_splash.png");
        assert_eq!(
            fs::read(icon).unwrap(),
            fs::read(public.join("icon.png")).unwrap()
        );
        assert_eq!(
            fs::read(splash).unwrap(),
            fs::read(public.join("splash.png")).unwrap()
        );
        let catalog = apple.join("Launch.xcassets");
        let icon = image::open(catalog.join("AppIcon.appiconset/icon.png"))
            .unwrap()
            .into_rgb8();
        assert_eq!(icon.dimensions(), (1024, 1024));
        assert_eq!(icon.get_pixel(0, 0).0, [tint, 0, 0]);
        for scale in [1, 2, 3] {
            let splash =
                image::open(catalog.join(format!("EweSplash.imageset/splash-{scale}x.png")))
                    .unwrap()
                    .into_rgba8();
            assert_eq!(splash.dimensions(), (96 * scale, 144 * scale));
            assert_eq!(splash.get_pixel(0, 0).0, [0, tint, 0, 128]);
        }
        let contents: serde_json::Value = serde_json::from_slice(
            &fs::read(catalog.join("EweLaunchBackground.colorset/Contents.json")).unwrap(),
        )
        .unwrap();
        for (index, color) in [(0, light), (1, dark)] {
            let red = u8::from_str_radix(&color[1..3], 16).unwrap();
            assert_eq!(
                contents["colors"][index]["color"]["components"]["red"],
                format!("{:.6}", f64::from(red) / 255.)
            );
        }
        assert_eq!(contents["colors"][1]["appearances"][0]["value"], "dark");
        assert!(catalog.join("Contents.json").is_file());
    }
}

#[test]
fn invalid_input_reports_the_source_path_and_preserves_the_last_build() {
    let root = tempfile::tempdir().unwrap();
    let public = root.path().join("public");
    let destination = root.path().join("android");
    inputs(&public, 40, "#FFEEDD", "#112233");
    launch::stage(&public, Target::Android, &destination).unwrap();
    for (name, bytes) in [
        (
            "launch-theme.toml",
            b"[background]\nlight='red'\ndark='#000000'".as_slice(),
        ),
        ("splash.png", b"bad"),
        ("icon.png", b"bad"),
    ] {
        let path = public.join(name);
        let original = fs::read(&path).unwrap();
        fs::write(&path, bytes).unwrap();
        let error = launch::stage(&public, Target::Android, &destination)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(
            fs::read_to_string(destination.join("res/values/ewe_launch_colors.xml"))
                .unwrap()
                .contains("#FFEEDD")
        );
        fs::write(&path, original).unwrap();
    }
    RgbaImage::from_pixel(16, 16, Rgba([1, 2, 3, 0]))
        .save(public.join("icon.png"))
        .unwrap();
    assert!(launch::stage(&public, Target::Apple, &destination)
        .unwrap_err()
        .to_string()
        .contains("icon.png"));
}
