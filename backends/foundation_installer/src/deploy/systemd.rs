//! The `systemd` deploy target: a unit file under
//! `<systemd_unit_dir>/ewe-<name>.service`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use foundation_packager::FileContent;
use serde::Serialize;

use crate::context::{CommandOutput, InstallContext};
use crate::deploy::{DeployTargetAdapter, Deployed};
use crate::error::InstallError;
use crate::manifest::{DeploySpec, Entry, SystemdRestart};
use crate::source::Artifact;

/// The unit template. It is rendered with `foundation_packager`.
const SYSTEMD_TEMPLATE: &str = include_str!("../../templates/systemd.service.j2");

/// The template name inside the Jinja environment.
const SYSTEMD_TEMPLATE_NAME: &str = "systemd.service.j2";

/// The default OS user that runs a systemd entry.
const DEFAULT_USER: &str = "ewe";

/// The `systemd` deploy adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemdDeploy;

#[async_trait]
impl DeployTargetAdapter for SystemdDeploy {
    fn kind(&self) -> &'static str {
        "systemd"
    }

    async fn apply(
        &self,
        entry: &Entry,
        artifact: &Artifact,
        previous: Option<&Deployed>,
        ctx: &InstallContext,
    ) -> Result<Deployed, InstallError> {
        let DeploySpec::Systemd {
            args,
            env,
            user,
            restart,
        } = &entry.deploy
        else {
            return Err(InstallError::Incompatible {
                entry: entry.name.to_string(),
                deploy: entry.deploy.kind().to_string(),
                artifact: artifact.class().to_string(),
            });
        };

        let (program, exec_path) = match artifact {
            Artifact::File { path, .. } => install_file(entry, path, ctx)?,
            Artifact::Tool { path, .. } => (path.clone(), path.clone()),
            Artifact::Image(_) => {
                return Err(InstallError::Incompatible {
                    entry: entry.name.to_string(),
                    deploy: self.kind().to_string(),
                    artifact: artifact.class().to_string(),
                });
            }
        };
        let exec_start = command_line(&program, args);

        let user = user.clone().unwrap_or_else(|| DEFAULT_USER.to_string());
        let restart = restart.unwrap_or(SystemdRestart::OnFailure);
        ensure_user(entry, &user, ctx).await?;

        let unit = unit_name(entry);
        let unit_path = ctx.config().systemd_unit_dir.join(&unit);
        std::fs::create_dir_all(&ctx.config().systemd_unit_dir).map_err(|err| {
            InstallError::Apply {
                entry: entry.name.to_string(),
                reason: format!(
                    "cannot make {}: {err}",
                    ctx.config().systemd_unit_dir.display()
                ),
            }
        })?;
        render_unit(entry, &unit_path, &exec_start, &user, restart, env, ctx)?;

        run_systemctl(entry, ctx, &["daemon-reload"]).await?;

        let start = if previous.is_some() {
            run_systemctl(entry, ctx, &["restart", &unit]).await
        } else {
            run_systemctl(entry, ctx, &["enable", "--now", &unit]).await
        };
        if let Err(err) = start {
            rollback(entry, previous, &exec_start, &user, restart, env, ctx).await;
            return Err(err);
        }

        if !wait_active(entry, &unit, ctx).await? {
            rollback(entry, previous, &exec_start, &user, restart, env, ctx).await;
            return Err(InstallError::StartTimeout {
                entry: entry.name.to_string(),
                timeout: ctx.config().start_timeout,
            });
        }

        Ok(Deployed::Systemd {
            unit,
            exec_path,
            version: entry.version.clone(),
        })
    }

    async fn exists(
        &self,
        _entry: &Entry,
        deployed: &Deployed,
        ctx: &InstallContext,
    ) -> Result<bool, InstallError> {
        match deployed {
            Deployed::Systemd { unit, .. } => {
                Ok(ctx.config().systemd_unit_dir.join(unit).is_file())
            }
            _ => Ok(false),
        }
    }
}

fn unit_name(entry: &Entry) -> String {
    format!("ewe-{}.service", entry.name)
}

/// The quoted `ExecStart` line: the program and each argument as one systemd
/// argument.
fn command_line(program: &Path, args: &[String]) -> String {
    let mut line = quote(&program.display().to_string());
    for arg in args {
        line.push(' ');
        line.push_str(&quote(arg));
    }
    line
}

/// Copy a downloaded file into its version directory and point `current` at
/// it. It gives the path for `ExecStart` and the versioned path for rollback.
fn install_file(
    entry: &Entry,
    source: &Path,
    ctx: &InstallContext,
) -> Result<(PathBuf, PathBuf), InstallError> {
    let version_directory = ctx
        .config()
        .install_root
        .join(entry.name.as_str())
        .join(&entry.version);
    std::fs::create_dir_all(&version_directory).map_err(|err| InstallError::Apply {
        entry: entry.name.to_string(),
        reason: format!("cannot make {}: {err}", version_directory.display()),
    })?;
    let installed = version_directory.join(entry.name.as_str());
    std::fs::copy(source, &installed).map_err(|err| InstallError::Apply {
        entry: entry.name.to_string(),
        reason: format!(
            "cannot copy {} to {}: {err}",
            source.display(),
            installed.display()
        ),
    })?;
    make_executable(&installed).map_err(|err| InstallError::Apply {
        entry: entry.name.to_string(),
        reason: format!("cannot set the mode of {}: {err}", installed.display()),
    })?;

    let current = ctx
        .config()
        .install_root
        .join(entry.name.as_str())
        .join("current");
    point_current(&current, &version_directory).map_err(|err| InstallError::Apply {
        entry: entry.name.to_string(),
        reason: format!(
            "cannot point {} at {}: {err}",
            current.display(),
            version_directory.display()
        ),
    })?;

    Ok((current.join(entry.name.as_str()), installed))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "systemd needs a Unix host",
    ))
}

/// Point the `current` link at `target` in one rename.
#[cfg(unix)]
fn point_current(link: &Path, target: &Path) -> std::io::Result<()> {
    let temporary = link.with_extension("tmp");
    if temporary.exists() {
        std::fs::remove_file(&temporary)?;
    }
    std::os::unix::fs::symlink(target, &temporary)?;
    std::fs::rename(&temporary, link)
}

#[cfg(not(unix))]
fn point_current(_link: &Path, _target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "systemd needs a Unix host",
    ))
}

#[derive(Debug, Serialize)]
struct UnitParams {
    name: String,
    version: String,
    user: String,
    exec_start: String,
    env: Vec<String>,
    restart: String,
    start_timeout_secs: u64,
}

fn render_unit(
    entry: &Entry,
    unit_path: &Path,
    exec_start: &str,
    user: &str,
    restart: SystemdRestart,
    env: &std::collections::BTreeMap<String, String>,
    ctx: &InstallContext,
) -> Result<(), InstallError> {
    let rendered_version = escape(&entry.version);
    let mut environment = minijinja::Environment::new();
    environment
        .add_template(SYSTEMD_TEMPLATE_NAME, SYSTEMD_TEMPLATE)
        .map_err(|err| InstallError::Apply {
            entry: entry.name.to_string(),
            reason: format!("cannot load the systemd unit template: {err}"),
        })?;
    let content = FileContent::Jinja(SYSTEMD_TEMPLATE_NAME.to_string(), Arc::new(environment));
    let params = UnitParams {
        name: entry.name.to_string(),
        version: rendered_version,
        user: escape(user),
        exec_start: exec_start.to_string(),
        env: env
            .iter()
            .map(|(key, value)| quote(&format!("{key}={value}")))
            .collect(),
        restart: restart.as_str().to_string(),
        start_timeout_secs: ctx.config().start_timeout.as_secs(),
    };
    content
        .run(unit_path, Some(&params))
        .map_err(|err| InstallError::Apply {
            entry: entry.name.to_string(),
            reason: format!("cannot render {}: {err:?}", unit_path.display()),
        })
}

/// Escape a value for a systemd unit file. `%` starts a specifier, and a
/// newline would break the line.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '%' => out.push_str("%%"),
            other => out.push(other),
        }
    }
    out
}

fn quote(value: &str) -> String {
    format!("\"{}\"", escape(value))
}

async fn ensure_user(entry: &Entry, user: &str, ctx: &InstallContext) -> Result<(), InstallError> {
    if user == "root" {
        return Ok(());
    }
    let exists = ctx
        .commands()?
        .run("id", &["-u".to_string(), user.to_string()])
        .await;
    match exists {
        Ok(output) if output.success() => Ok(()),
        _ => {
            let output = ctx
                .commands()?
                .run(
                    "useradd",
                    &[
                        "--system".to_string(),
                        "--no-create-home".to_string(),
                        "--shell".to_string(),
                        "/usr/sbin/nologin".to_string(),
                        user.to_string(),
                    ],
                )
                .await
                .map_err(|err| InstallError::Apply {
                    entry: entry.name.to_string(),
                    reason: err.to_string(),
                })?;
            if output.success() {
                Ok(())
            } else {
                Err(InstallError::Apply {
                    entry: entry.name.to_string(),
                    reason: format!("cannot make the user {user}: {}", output.stderr.trim()),
                })
            }
        }
    }
}

async fn run_systemctl(
    entry: &Entry,
    ctx: &InstallContext,
    args: &[&str],
) -> Result<CommandOutput, InstallError> {
    let args: Vec<String> = args.iter().map(|value| (*value).to_string()).collect();
    ctx.commands()?
        .run("systemctl", &args)
        .await
        .map_err(|err| InstallError::Apply {
            entry: entry.name.to_string(),
            reason: err.to_string(),
        })
}

async fn wait_active(
    entry: &Entry,
    unit: &str,
    ctx: &InstallContext,
) -> Result<bool, InstallError> {
    let deadline = ctx.context().clock().now() + ctx.config().start_timeout;
    loop {
        let output = run_systemctl(entry, ctx, &["is-active", unit]).await?;
        if output.success() && output.stdout.trim() == "active" {
            return Ok(true);
        }
        if ctx.context().clock().now() >= deadline {
            return Ok(false);
        }
        ctx.sleep(ctx.config().start_poll_interval).await;
    }
}

/// Put the previous version back after a failed start. A first install stops
/// the unit and removes its file.
async fn rollback(
    entry: &Entry,
    previous: Option<&Deployed>,
    exec_start: &str,
    user: &str,
    restart: SystemdRestart,
    env: &std::collections::BTreeMap<String, String>,
    ctx: &InstallContext,
) {
    let Some(Deployed::Systemd {
        unit,
        exec_path,
        version,
    }) = previous
    else {
        let _ = run_systemctl(entry, ctx, &["disable", "--now", &unit_name(entry)]).await;
        let _ = std::fs::remove_file(ctx.config().systemd_unit_dir.join(unit_name(entry)));
        return;
    };

    if let Some(version_directory) = exec_path.parent() {
        let name_root = ctx.config().install_root.join(entry.name.as_str());
        if version_directory.parent() == Some(name_root.as_path()) {
            let current = name_root.join("current");
            if let Err(err) = point_current(&current, version_directory) {
                tracing::error!(
                    entry = %entry.name,
                    error = %err,
                    "cannot point the current link back"
                );
            }
        }
    }

    let previous_entry = Entry {
        version: version.clone(),
        ..entry.clone()
    };
    if let Err(err) = render_unit(
        &previous_entry,
        &ctx.config().systemd_unit_dir.join(unit),
        exec_start,
        user,
        restart,
        env,
        ctx,
    ) {
        tracing::error!(entry = %entry.name, error = %err, "cannot restore the old unit file");
    }
    let _ = run_systemctl(entry, ctx, &["daemon-reload"]).await;
    if let Err(err) = run_systemctl(entry, ctx, &["restart", unit.as_str()]).await {
        tracing::error!(entry = %entry.name, error = %err, "cannot restart the old unit");
    }
}
