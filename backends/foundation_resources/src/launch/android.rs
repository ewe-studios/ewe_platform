use super::{write, Background};
use std::{fs, io, path::Path};

pub(super) fn write_assets(
    root: &Path,
    icon: &Path,
    splash: &Path,
    background: &Background,
) -> io::Result<()> {
    let res = root.join("res");
    for (directory, color) in [
        ("values", &background.light),
        ("values-night", &background.dark),
    ] {
        write(
            &res,
            &format!("{directory}/ewe_launch_colors.xml"),
            format!(
                r#"<resources><color name="ewe_launch_background">{color}</color></resources>"#
            ),
        )?;
    }
    let drawables = res.join("drawable-nodpi");
    fs::create_dir_all(&drawables)?;
    fs::copy(icon, drawables.join("ewe_launch_icon.png"))?;
    fs::copy(splash, drawables.join("ewe_launch_splash.png"))?;
    write(
        &res,
        "mipmap-anydpi-v26/ewe_launcher.xml",
        r#"<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android"><background android:drawable="@color/ewe_launch_background"/><foreground android:drawable="@drawable/ewe_launch_icon"/></adaptive-icon>"#,
    )?;
    Ok(())
}
