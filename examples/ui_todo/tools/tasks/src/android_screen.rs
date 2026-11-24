//! Read the Android accessibility tree and exercise the example screens.
use super::device::{cleanup, sleep};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use quick_xml::{events::Event, Reader};
use std::{collections::BTreeMap, path::Path, time::Duration};

type Bounds = [i32; 4];
type Nodes = BTreeMap<String, Bounds>;
pub(super) struct Screen<'a> {
    pub ctx: &'a TaskContext,
    pub adb: ProcessCommand,
    pub package: &'a str,
    pub timeout: Duration,
    pub interval: Duration,
}
fn bounds(s: &str) -> Result<Bounds, TaskError> {
    let values = s
        .split(|c: char| c != '-' && !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(TaskError::operation)?;
    values
        .try_into()
        .map_err(|_| TaskError::operation("Invalid accessibility bounds"))
}
fn nodes(xml: &[u8], package: &str) -> Result<Nodes, TaskError> {
    let mut reader = Reader::from_reader(xml);
    let mut out = Nodes::new();
    loop {
        match reader.read_event().map_err(TaskError::operation)? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "node" => {
                let mut attrs = BTreeMap::new();
                for attr in e.attributes() {
                    let attr = attr.map_err(TaskError::operation)?;
                    attrs.insert(
                        attr.key.as_ref().to_owned(),
                        attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(TaskError::operation)?
                            .into_owned(),
                    );
                }
                let attr = |key: &str| attrs.get(key).map_or("", String::as_str);
                if attr("package") == package {
                    let rect = bounds(attr("bounds"))?;
                    if attr("class") == "android.widget.EditText" {
                        out.entry("<editor>".into()).or_insert(rect);
                        if attr("focused") == "true" {
                            out.entry("<focused-editor>".into()).or_insert(rect);
                        }
                    }
                    for label in [attr("text"), attr("content-desc")] {
                        if !label.is_empty() {
                            out.entry(label.into()).or_insert(rect);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}
impl Screen<'_> {
    async fn shell(&self, args: &[&str]) -> Result<String, TaskError> {
        super::device::text(
            self.ctx,
            self.adb.clone().arg("shell").args(args.iter().copied()),
        )
        .await
    }
    async fn find(&self, args: &[&str], pattern: &str) -> Result<Option<String>, TaskError> {
        super::android_report::find(
            self.ctx,
            self.adb.clone().arg("shell").args(args.iter().copied()),
            pattern,
        )
        .await
    }
    async fn restore(&self, args: &[&str]) -> TaskResult {
        super::device::cleanup_command(
            self.ctx,
            self.adb.clone().arg("shell").args(args.iter().copied()),
        )
        .await
    }
    async fn wait(&self, present: &[&str], absent: &[&str]) -> Result<Nodes, TaskError> {
        let file = "/sdcard/ewe-screen.xml";
        let start = self.ctx.context().clock().elapsed();
        let mut visible = Nodes::new();
        let poll = async {
            loop {
                match self.shell(&["uiautomator", "dump", file]).await {
                    Ok(report) if report.contains(file) => {
                        let bytes = super::output::device_query(
                            self.ctx,
                            self.adb.clone().args(["shell", "cat", file]),
                        )
                        .await?;
                        visible = nodes(&bytes, self.package)?;
                        if present.iter().all(|name| visible.contains_key(*name))
                            && absent.iter().all(|name| !visible.contains_key(*name))
                        {
                            return Ok(visible);
                        }
                    }
                    Ok(_) | Err(TaskError::Exit { .. }) => {}
                    Err(e) => return Err(e),
                }
                if self.ctx.context().clock().elapsed().saturating_sub(start) >= self.timeout {
                    return Err(TaskError::operation(format!(
                        "{} requires {present:?} without {absent:?}. Visible labels: {:?}",
                        self.package,
                        visible.keys()
                    )));
                }
                sleep(self.ctx, self.interval).await?;
            }
        };
        let deadline = async {
            sleep(self.ctx, self.timeout).await?;
            Err(TaskError::operation(format!(
                "Timed out waiting for {present:?} in {}",
                self.package
            )))
        };
        futures_lite::future::race(poll, deadline).await
    }
    async fn tap(&self, label: &str, also: &[&str]) -> TaskResult {
        let mut present = vec![label];
        present.extend_from_slice(also);
        let rect = self.wait(&present, &[]).await?[label];
        self.shell(&[
            "input",
            "tap",
            &i32::midpoint(rect[0], rect[2]).to_string(),
            &i32::midpoint(rect[1], rect[3]).to_string(),
        ])
        .await?;
        Ok(())
    }
    async fn type_title(&self, title: &str) -> TaskResult {
        self.tap("Add", &["Todos"]).await?;
        self.tap("Todo title", &["New todo"]).await?;
        self.shell(&["input", "text", &title.replace(' ', "%s")])
            .await?;
        Ok(())
    }
    async fn create(&self, title: &str) -> TaskResult {
        self.type_title(title).await?;
        self.tap("Add", &["New todo"]).await?;
        self.wait(&[title], &["New todo"]).await?;
        Ok(())
    }
    async fn delete(&self, title: &str) -> TaskResult {
        self.tap("Delete", &[title]).await?;
        self.tap("Confirm", &["Delete this todo?"]).await?;
        self.wait(&["0 open todos"], &[title, "Delete this todo?"])
            .await?;
        Ok(())
    }
    pub async fn starter(&self, url: &str) -> TaskResult {
        self.wait(&["Open todos", "0", "Refresh"], &[]).await?;
        super::seed::run(self.ctx, url).await?;
        self.tap("Refresh", &["0"]).await?;
        self.wait(&["Open todos", "1", "Refresh"], &[]).await?;
        Ok(())
    }
    pub async fn todo(&self, mode: &str, captures: Option<&Path>) -> TaskResult {
        let local = "Android offline todo";
        let remote = "Android server todo";
        match mode {
            "offline" => {
                if self
                    .find(
                        &["dumpsys", "package", self.package],
                        r"android\.permission\.INTERNET",
                    )
                    .await?
                    .is_some()
                {
                    return Err(TaskError::operation("Offline app requests internet access"));
                }
                self.wait(
                    &["Todos", "0 open todos", "Add", "All", "Open", "Done"],
                    &[],
                )
                .await?;
                self.create(local).await?;
                self.wait(&[local, "1 open todo", "Complete"], &[]).await?;
                self.tap("Complete", &[local]).await?;
                self.wait(&["0 open todos", "Reopen"], &[]).await?;
                self.tap("Open", &[]).await?;
                self.wait(&["0 open todos"], &[local]).await?;
                self.tap("Done", &[]).await?;
                self.tap("Reopen", &[local]).await?;
                self.wait(&["1 open todo"], &[local]).await?;
                self.tap("Open", &[]).await?;
                self.delete(local).await?;
                self.create(local).await?;
                self.wait(&["1 open todo"], &[]).await?;
            }
            "server" => {
                self.wait(&["Todos", "Add", "0 open todos"], &[local, remote])
                    .await?;
                self.create(remote).await?;
                self.tap("Complete", &[remote, "1 open todo"]).await?;
                self.wait(&["0 open todos"], &[]).await?;
                self.tap("Done", &[]).await?;
                self.tap("Reopen", &[remote]).await?;
                self.wait(&["1 open todo"], &[]).await?;
                self.tap("Open", &[]).await?;
                self.wait(&[remote, "Complete"], &[local]).await?;
            }
            "server-delete" => self.delete(remote).await?,
            "isolation" => {
                self.wait(&[local, "1 open todo"], &[remote]).await?;
            }
            "theme-review" => {
                self.themes(
                    captures.ok_or_else(|| TaskError::operation("Set EWE_THEME_CAPTURES"))?,
                )
                .await?;
            }
            _ => {
                return Err(TaskError::operation(format!(
                    "Unknown Todo screen mode {mode}"
                )))
            }
        }
        Ok(())
    }
    async fn capture(&self, directory: &Path, name: &str) -> TaskResult {
        let device_file = "/sdcard/ewe-screen.png";
        self.shell(&["screencap", "-p", device_file]).await?;
        let output = self
            .ctx
            .exec(
                self.adb
                    .clone()
                    .args(["pull", device_file])
                    .arg(directory.join(format!("{name}.png"))),
            )
            .await;
        cleanup(output, self.restore(&["rm", "-f", device_file]).await)
    }
    async fn theme(&self, directory: &Path, mode: &str) -> TaskResult {
        let title = "Buy milk and collect the groceries for dinner";
        self.shell(&["input", "keyevent", "KEYCODE_WAKEUP"]).await?;
        self.shell(&["wm", "dismiss-keyguard"]).await?;
        self.shell(&["am", "force-stop", self.package]).await?;
        self.shell(&[
            "cmd",
            "uimode",
            "night",
            if mode == "dark" { "yes" } else { "no" },
        ])
        .await?;
        self.shell(&[
            "settings",
            "put",
            "system",
            "font_scale",
            if mode == "branded" { "1.4" } else { "1.0" },
        ])
        .await?;
        self.shell(&[
            "am",
            "start",
            "-n",
            &format!("{}/.MainActivity", self.package),
            "--ez",
            "branded",
            if mode == "branded" { "true" } else { "false" },
        ])
        .await?;
        self.type_title(title).await?;
        let controls = self.wait(&[title, "Cancel", "Add"], &[]).await?;
        let (cancel, add) = (controls["Cancel"], controls["Add"]);
        if cancel[3].min(add[3]) <= cancel[1].max(add[1])
            || self
                .find(&["dumpsys", "input_method"], "mInputShown=true")
                .await?
                .is_none()
        {
            return Err(TaskError::operation(
                "Input controls or keyboard are not visible",
            ));
        }
        let top = self
            .find(
                &["dumpsys", "window"],
                r"type=ime frame=\[\d+,(\d+)\]\[\d+,\d+\][^\n]* visible=true",
            )
            .await?
            .and_then(|v| v.parse::<i32>().ok())
            .ok_or_else(|| TaskError::operation("Missing keyboard bounds"))?;
        if cancel[3].max(add[3]) > top {
            return Err(TaskError::operation("Keyboard obscures entry controls"));
        }
        self.capture(directory, &format!("android-{mode}-entry"))
            .await?;
        self.tap("Cancel", &[title]).await?;
        self.wait(&["0 open todos"], &[title, "New todo"]).await?;
        self.type_title(title).await?;
        if mode == "branded" {
            self.tap("Add", &[title]).await?;
        } else {
            self.shell(&["input", "keyevent", "KEYCODE_ENTER"]).await?;
        }
        self.wait(&[title, "Complete"], &["New todo"]).await?;
        self.tap("Complete", &[]).await?;
        self.wait(&[title, "Reopen"], &[]).await?;
        self.capture(directory, &format!("android-{mode}-list"))
            .await?;
        self.tap("Delete", &[]).await?;
        let controls = self
            .wait(&["Confirm", "Cancel", "Delete this todo?"], &[])
            .await?;
        let (cancel, confirm) = (controls["Cancel"], controls["Confirm"]);
        if cancel[3].min(confirm[3]) <= cancel[1].max(confirm[1])
            || cancel[2] > confirm[0] && confirm[2] > cancel[0]
        {
            return Err(TaskError::operation("Confirmation controls overlap"));
        }
        self.capture(directory, &format!("android-{mode}-confirmation"))
            .await?;
        self.tap("Cancel", &[]).await?;
        self.wait(&[title], &["Delete this todo?"]).await?;
        self.delete(title).await
    }
    async fn themes(&self, directory: &Path) -> TaskResult {
        std::fs::create_dir_all(directory).map_err(TaskError::operation)?;
        let timeout = self
            .shell(&["settings", "get", "system", "screen_off_timeout"])
            .await?;
        let scale = self
            .shell(&["settings", "get", "system", "font_scale"])
            .await?;
        let night = self.shell(&["cmd", "uimode", "night"]).await?;
        let night = night
            .split_whitespace()
            .last()
            .ok_or_else(|| TaskError::operation("Missing current night setting"))?;
        let result = async {
            self.shell(&[
                "settings",
                "put",
                "system",
                "screen_off_timeout",
                &self.timeout.as_millis().saturating_mul(10).to_string(),
            ])
            .await?;
            for mode in ["light", "dark", "branded"] {
                self.theme(directory, mode).await?;
            }
            Ok(())
        }
        .await;
        let mut result = result;
        for (key, val) in [("screen_off_timeout", timeout), ("font_scale", scale)] {
            let args = if val == "null" {
                vec!["settings", "delete", "system", key]
            } else {
                vec!["settings", "put", "system", key, val.as_str()]
            };
            result = cleanup(result, self.restore(&args).await);
        }
        cleanup(
            result,
            self.shell(&["cmd", "uimode", "night", night])
                .await
                .map(drop),
        )
    }
}

#[derive(clap::Args)]
pub(super) struct Peer {
    #[arg(long)]
    adb: std::path::PathBuf,
    #[arg(long)]
    device: String,
    #[arg(value_enum)]
    phase: PeerPhase,
    labels: Vec<String>,
}
#[derive(Clone, Copy, clap::ValueEnum)]
enum PeerPhase {
    Ready,
    Draft,
    CheckDraft,
    Check,
    ShowAll,
    Add,
    PendingAdd,
    Retry,
    Complete,
    Reopen,
    Delete,
}
pub(super) async fn peer(ctx: TaskContext, args: Peer) -> TaskResult {
    let env = ctx.require::<super::NativeBuildEnv>()?;
    let secs = env
        .devices
        .get("EWE_ANDROID_VIEW_TIMEOUT")
        .and_then(|v| v.to_str())
        .map(str::parse::<u64>)
        .transpose()
        .map_err(TaskError::operation)?
        .unwrap_or(60);
    let screen = Screen {
        ctx: &ctx,
        adb: ProcessCommand::new(args.adb).args(["-s", &args.device]),
        package: "ewe.todo",
        timeout: Duration::from_secs(secs),
        interval: Duration::from_millis(500),
    };
    let label = |index: usize| {
        args.labels
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| TaskError::operation("Missing peer screen label"))
    };
    match args.phase {
        PeerPhase::Ready => {
            screen.wait(&["Todos", "Add", "0 open todos"], &[]).await?;
        }
        PeerPhase::Draft => {
            screen.tap("Done", &[]).await?;
            screen.tap("Add", &["Todos"]).await?;
            screen
                .wait(&["New todo", "Todo title", "Cancel"], &[])
                .await?;
        }
        PeerPhase::CheckDraft => {
            screen
                .wait(&["New todo", "Todo title", "Cancel"], &[])
                .await?;
            screen.tap("Cancel", &[]).await?;
            screen.wait(&["Done"], &[label(0)?, "New todo"]).await?;
        }
        PeerPhase::Check => {
            let (title, state) = (label(0)?, label(1)?);
            if state == "absent" {
                screen.wait(&["Todos"], &[title]).await?;
            } else {
                screen.wait(&[title, state], &[]).await?;
            }
        }
        PeerPhase::ShowAll => screen.tap("All", &[]).await?,
        PeerPhase::Add | PeerPhase::PendingAdd => {
            let title = label(0)?;
            add_peer(&screen, title, matches!(args.phase, PeerPhase::PendingAdd)).await?;
        }
        PeerPhase::Retry | PeerPhase::Complete | PeerPhase::Reopen => {
            let (tap, next) = match args.phase {
                PeerPhase::Retry => ("Try again", "Complete"),
                PeerPhase::Complete => ("Complete", "Reopen"),
                _ => ("Reopen", "Complete"),
            };
            screen.tap(tap, &[label(0)?]).await?;
            screen.wait(&[label(0)?, next], &[]).await?;
        }
        PeerPhase::Delete => {
            screen.tap("Delete", &[label(0)?]).await?;
            screen.tap("Confirm", &["Delete this todo?"]).await?;
            screen
                .wait(&["Todos"], &[label(0)?, "Delete this todo?"])
                .await?;
        }
    }
    Ok(())
}

async fn add_peer(screen: &Screen<'_>, title: &str, pending: bool) -> TaskResult {
    if !title
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
    {
        return Err(TaskError::operation(
            "Peer titles accept ASCII letters, digits, spaces, hyphens, and underscores",
        ));
    }
    screen.tap("Add", &["Todos"]).await?;
    screen.tap("<editor>", &["New todo"]).await?;
    super::device::wait_for(
        screen.ctx,
        screen.timeout,
        screen.interval,
        "editor focus",
        || async {
            let visible = screen.wait(&["<editor>"], &[]).await?;
            Ok(visible.contains_key("<focused-editor>")
                || screen
                    .find(&["dumpsys", "input_method"], "mInputShown=true")
                    .await?
                    .is_some())
        },
    )
    .await?;
    for (index, c) in title.char_indices() {
        screen
            .shell(&[
                "input",
                "text",
                &if c == ' ' {
                    "%s".to_owned()
                } else {
                    c.to_string()
                },
            ])
            .await?;
        if c != ' ' {
            screen
                .wait(&[&title[..index + c.len_utf8()], "New todo"], &[])
                .await?;
        }
    }
    screen.tap("Add", &["New todo"]).await?;
    if pending {
        screen.wait(&[title, "Pending"], &["New todo"]).await?;
    } else {
        screen.wait(&[title], &["New todo"]).await?;
    }
    Ok(())
}
