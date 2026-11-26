"""Exercise embedded task scripts from installed development and release binaries."""

from pathlib import Path
import argparse
import os
import signal
import subprocess
import sys
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("workspace", type=Path)
options = parser.parse_args()
workspace = options.workspace.resolve()
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    project = root / "owner"
    scripts = project / "src/tasks/scripts"
    scripts.mkdir(parents=True)
    (project / "Cargo.toml").write_text(f'''[package]
name = "packed-task-check"
edition = "2021"
version = "0.1.0"
[workspace]
[dependencies]
clap = {{ version = "4", features = ["derive"] }}
foundation_tasks = {{ path = "{workspace}/backends/foundation_tasks", features = ["scripts"] }}
foundation_macros = {{ path = "{workspace}/backends/foundation_macros" }}
foundation_nostd = {{ path = "{workspace}/backends/foundation_nostd" }}
''')
    (project / "build.rs").write_text(
        'fn main(){println!("cargo:rerun-if-changed=src/tasks/scripts");}'
    )
    (project / "src/main.rs").write_text(r"""
use foundation_macros::EmbedDirectoryAs;
use foundation_tasks::scripts::{run, ScriptPlatform, ScriptVariant};
use foundation_tasks::{task, ProcessCommand, TaskContext, TaskResult, TaskRunner, TaskSet};
#[derive(EmbedDirectoryAs, Default)]
#[source = "$CURRENT_CRATE/src/tasks/scripts/"]
#[always_embed]
struct Files;
#[derive(clap::Args)]
struct Args {
    interpreter: std::ffi::OsString,
    output: std::ffi::OsString,
    mode: String,
}
async fn execute(ctx: TaskContext, args: Args) -> TaskResult {
    let entries = [
        ScriptVariant {
            platform: Some(ScriptPlatform::Linux),
            entry: "linux.py",
        },
        ScriptVariant {
            platform: Some(ScriptPlatform::Android),
            entry: "android.py",
        },
        ScriptVariant {
            platform: None,
            entry: "shared.py",
        },
    ];
    let platform = match args.mode.as_str() {
        "android" => ScriptPlatform::Android,
        "linux" => ScriptPlatform::Linux,
        "unsupported" => ScriptPlatform::Ios,
        _ => ScriptPlatform::host()?,
    };
    let entries = if args.mode == "unsupported" {
        &entries[..2]
    } else {
        &entries[..]
    };
    run(
        &ctx,
        &Files,
        entries,
        platform,
        ProcessCommand::new(args.interpreter),
        &[args.output, args.mode.into()],
    )
    .await
}
#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    let mut set = TaskSet::new("owner");
    set.add("probe", "Run embedded package checks", execute)?;
    runner.register(set)?;
    runner.run().await
}
""")
    (scripts / "part").mkdir()
    (scripts / "part/item.txt").write_text("first")
    for name in ["shared", "linux", "android"]:
        (scripts / (name + ".py")).write_text("""from pathlib import Path
import sys,time
root=Path(__file__).parent
Path(sys.argv[1]).write_text(str(root)+"\\n"+Path(__file__).stem+" "+(root/"part/item.txt").read_text()+" "+str((root/"added").exists())+" "+str((root/"removed").exists()))
if sys.argv[2]=="wait":time.sleep(120)
if sys.argv[2]=="fail":sys.exit(23)
""")
    (scripts / "removed").write_text("obsolete")
    outside = root / "outside"
    outside.mkdir()
    cache = os.environ.copy()
    cache["CARGO_NET_OFFLINE"] = "true"
    for pass_number, profile in enumerate(["dev", "dev", "release"]):
        dest = root / profile

        def install():
            subprocess.run(
                [
                    "cargo",
                    "install",
                    "--offline",
                    "--force",
                    "--profile",
                    profile,
                    "--path",
                    project,
                    "--root",
                    dest,
                ],
                cwd=workspace,
                env=cache,
                check=True,
            )

        def probe(mode):
            result = outside / "result.txt"
            result.unlink(missing_ok=True)
            return [
                dest
                / "bin"
                / ("packed-task-check.exe" if os.name == "nt" else "packed-task-check"),
                "task",
                "run",
                "probe",
                sys.executable,
                result,
                mode,
            ], result

        install()
        hidden = root / "hidden"
        scripts.rename(hidden)
        try:
            for mode in ["linux", "android", "host", "fail", "unsupported"]:
                cmd, result = probe(mode)
                outcome = subprocess.run(
                    cmd, cwd=outside, capture_output=True, text=True, timeout=20
                )
                expected = {"fail": 23, "unsupported": 1}.get(mode, 0)
                assert outcome.returncode == expected, (outcome.stdout, outcome.stderr)
                if mode == "unsupported":
                    assert not result.exists()
                    continue
                tempdir, record = result.read_text().splitlines()
                assert not Path(tempdir).exists()
                if mode in ["linux", "android"]:
                    assert record.startswith(mode + " ")
                if pass_number:
                    assert "second True False" in record
                else:
                    assert "first False True" in record
            if os.name != "nt":
                cmd, result = probe("wait")
                child = subprocess.Popen(cmd, cwd=outside)
                try:
                    deadline = time.monotonic() + 8
                    while not result.is_file() and time.monotonic() < deadline:
                        time.sleep(0.05)
                    tempdir = result.read_text().splitlines()[0]
                    child.send_signal(signal.SIGINT)
                    assert child.wait(timeout=10) == 130 and not Path(tempdir).exists()
                finally:
                    if child.poll() is None:
                        child.kill()
                        child.wait()
        finally:
            hidden.rename(scripts)
        if pass_number == 0:
            (scripts / "part/item.txt").write_text("second")
            (scripts / "added").write_text("new")
            (scripts / "removed").unlink()
    print(
        "Installed scripts, companion files, selection, updates, errors, and cancellation passed."
    )
