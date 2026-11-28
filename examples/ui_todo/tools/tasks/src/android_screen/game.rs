//! Replay a seeded Mathalley game and inspect the frames presented by Android.
use super::{cleanup, sleep, Screen};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(long)]
    adb: PathBuf,
    #[arg(long)]
    device: String,
    #[arg(long)]
    captures: PathBuf,
    #[arg(long, default_value_t = 60)]
    timeout_seconds: u64,
    #[arg(long, default_value_t = 200)]
    poll_ms: u64,
}

struct Scene {
    left: u32,
    width: u32,
    height: u32,
    top: u32,
}

impl Scene {
    fn point(&self, x: u32, y: u32) -> (u64, u64) {
        let width = u64::from(self.width);
        let height = u64::from(self.height);
        // The game uses a centered 390 by 844 canvas. Keep its aspect ratio.
        let (scale, unit) = if width * 844 <= height * 390 {
            (width, 390)
        } else {
            (height, 844)
        };
        let axis = |available, extent, position| {
            let margin = available * unit - extent * scale;
            (margin + 2 * scale * u64::from(position) + unit) / (2 * unit)
        };
        (
            u64::from(self.left) + axis(width, 390, x),
            u64::from(self.top) + axis(height, 844, y),
        )
    }
}

fn sample(png: &Path, rgb: &Path, color: Option<[u8; 3]>) -> io::Result<Option<Scene>> {
    let mut image = File::open(png)?;
    image.seek(SeekFrom::Start(16))?;
    let mut size = [0; 4];
    image.read_exact(&mut size)?;
    let width = u32::from_be_bytes(size);
    image.read_exact(&mut size)?;
    let height = u32::from_be_bytes(size);
    let mut pixels = File::open(rgb)?;
    let mut pixel = |x: u64, y: u64| -> io::Result<[u8; 3]> {
        pixels.seek(SeekFrom::Start(3 * (u64::from(width) * y + x)))?;
        let mut value = [0; 3];
        pixels.read_exact(&mut value)?;
        Ok(value)
    };
    // The window can leave navigation bars on any side of the game.
    let background = [0x1b, 0x14, 0x10];
    let mut rows = 0..height;
    let mut edge = |row| pixel(u64::from(width / 4), u64::from(row));
    let top = loop {
        let Some(row) = rows.next() else {
            return Ok(None);
        };
        if edge(row)? == background {
            break row;
        }
    };
    let bottom = loop {
        let Some(row) = rows.next_back() else {
            return Ok(None);
        };
        if edge(row)? == background {
            break row + 1;
        }
    };
    // The screen can have landscape dimensions while Android still rotates the view.
    // Check the opposite edge before using the surface bounds for a tap.
    let opposite = u64::from(width - width / 4);
    if width > height
        && (pixel(opposite, u64::from(top))? != background
            || pixel(opposite, u64::from(bottom - 1))? != background)
    {
        return Ok(None);
    }
    let mut row = vec![0; width as usize * 3];
    pixels.seek(SeekFrom::Start(3 * u64::from(width) * u64::from(top)))?;
    pixels.read_exact(&mut row)?;
    let Some(left) = row.chunks_exact(3).position(|rgb| rgb == background) else {
        return Ok(None);
    };
    let Some(right) = row.chunks_exact(3).rposition(|rgb| rgb == background) else {
        return Ok(None);
    };
    let scene = Scene {
        left: left as u32,
        width: (right - left + 1) as u32,
        top,
        height: bottom - top,
    };
    let (x, y) = scene.point(70, 200);
    pixels.seek(SeekFrom::Start(3 * (u64::from(width) * y + x)))?;
    let mut actual = [0; 3];
    pixels.read_exact(&mut actual)?;
    Ok(color
        .is_none_or(|expected| expected == actual)
        .then_some(scene))
}

async fn presented(
    screen: &Screen<'_>,
    captures: &Path,
    name: &str,
    color: Option<[u8; 3]>,
    landscape: bool,
) -> Result<Scene, TaskError> {
    let start = screen.ctx.context().clock().elapsed();
    let scratch = tempfile::tempdir_in(captures).map_err(TaskError::operation)?;
    let png = scratch.path().join("frame.png");
    let rgb = scratch.path().join("frame.rgb");
    loop {
        screen.capture(scratch.path(), "frame").await?;
        screen
            .ctx
            .exec(
                ProcessCommand::new("magick")
                    .arg(&png)
                    .args(["-depth", "8"])
                    .arg(&rgb),
            )
            .await?;
        let source = png.clone();
        let pixels = rgb.clone();
        if let Some(scene) = blocking::unblock(move || sample(&source, &pixels, color))
            .await
            .map_err(TaskError::operation)?
        {
            if !landscape || scene.width > scene.height {
                std::fs::copy(&png, captures.join(format!("{name}.png")))
                    .map_err(TaskError::operation)?;
                return Ok(scene);
            }
        }
        if screen.ctx.context().clock().elapsed().saturating_sub(start) >= screen.timeout {
            return Err(TaskError::operation(format!(
                "Android did not display {name}"
            )));
        }
        sleep(screen.ctx, screen.interval).await?;
    }
}

async fn touch(screen: &Screen<'_>, scene: &Scene, x: u32, y: u32) -> TaskResult {
    let (x, y) = scene.point(x, y);
    screen
        .shell(&["input", "tap", &x.to_string(), &y.to_string()])
        .await
        .map(drop)
}

pub(crate) async fn run(ctx: TaskContext, args: Options) -> TaskResult {
    let screen = Screen {
        ctx: &ctx,
        adb: ProcessCommand::new(args.adb).args(["-s", &args.device]),
        package: "ewe.todo.local",
        timeout: Duration::from_secs(args.timeout_seconds),
        interval: Duration::from_millis(args.poll_ms),
    };
    std::fs::create_dir_all(&args.captures).map_err(TaskError::operation)?;
    screen
        .shell(&[
            "am",
            "start",
            "-S",
            "-n",
            "ewe.todo.local/.MainActivity",
            "--es",
            "gameSeed",
            "13",
        ])
        .await?;
    screen.tap("Play Mathalley", &[]).await?;
    screen.wait(&["Back to Todos"], &[]).await?;
    let scene = presented(&screen, &args.captures, "android-title", None, false).await?;
    touch(&screen, &scene, 195, 422).await?;
    let train = presented(
        &screen,
        &args.captures,
        "android-game",
        Some([0x27, 0x46, 0x53]),
        false,
    )
    .await?;
    for (index, x) in [75, 195, 315, 315, 75, 315].into_iter().enumerate() {
        touch(&screen, &train, x, 660).await?;
        sleep(
            &ctx,
            Duration::from_millis(if index == 2 { 2500 } else { 450 }),
        )
        .await?;
    }
    let color = Some([0xc6, 0x3f, 0x39]);
    presented(&screen, &args.captures, "android-gumball", color, false).await?;
    let rotation = screen.shell(&["wm", "user-rotation"]).await?;
    let mut restore = vec!["wm", "user-rotation"];
    restore.extend(rotation.split_whitespace());
    screen.shell(&["wm", "user-rotation", "lock", "1"]).await?;
    let result = async {
        // Wait for the dialog, then read the surface used for the answer tap.
        screen
            .wait(&["Back to Todos", "Mathalley game"], &[])
            .await?;
        let scene = presented(
            &screen,
            &args.captures,
            "android-gumball-landscape",
            color,
            true,
        )
        .await?;
        touch(&screen, &scene, 105, 690).await?;
        // Keep the display in landscape until the game processes the answer.
        presented(
            &screen,
            &args.captures,
            "android-gumball-landscape-result",
            Some([0xfd, 0xf3, 0xdc]),
            true,
        )
        .await?;
        screen.tap("Back to Todos", &[]).await
    }
    .await;
    cleanup(result, screen.restore(&restore).await)?;
    screen.wait(&["Play Mathalley"], &[]).await?;
    Ok(())
}
