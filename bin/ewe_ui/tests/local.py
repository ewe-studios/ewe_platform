"""Verify task dispatch from two independent generated applications."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("task_library", type=Path)
args = parser.parse_args()
binary = args.binary.resolve()
library = args.task_library.resolve()
with tempfile.TemporaryDirectory() as directory:
    base = Path(directory)
    for name in ["Ledger", "Orchard"]:
        subprocess.run([binary, "new", "--name", name, "--target", "web"], cwd=base, check=True)
        root = base / name
        tasks = root / "tools/tasks"
        (tasks / "src").mkdir(parents=True)
        (tasks / "Cargo.toml").write_text(f"""[package]
name = "{name.lower()}_tasks"
edition = "2021"
version = "0.1.0"
[workspace]
[dependencies]
foundation_tasks = {{ path = {json.dumps(str(library))} }}
clap = {{ version = "4", features = ["derive"] }}
""")
        (tasks / "src/lib.rs").write_text('''
use foundation_tasks::{TaskContext,TaskError,TaskResult,TaskSet};
#[derive(clap::Args)]
struct Marker { #[arg(long)] value: String, #[arg(long)] fail: bool }
async fn mark(_:TaskContext,args:Marker)->TaskResult {
    if args.fail { return Err(TaskError::Exit{program:"marker".into(),code:19}); }
    std::fs::write("result.txt",args.value).map_err(TaskError::operation)
}
pub fn tasks()->Result<TaskSet,TaskError> {
    let mut set=TaskSet::new("local marker").with_namespace("sample");
    set.add("mark","Write the requested marker",mark)?;
    Ok(set)
}
''')
        (tasks / "src/main.rs").write_text(f'''
use foundation_tasks::{{task,TaskResult,TaskRunner}};
#[task]
async fn main(mut runner:TaskRunner)->TaskResult {{
    runner.register({name.lower()}_tasks::tasks()?)?;
    runner.run().await
}}
''')
        with (root / "ewe-ui.toml").open("a") as manifest:
            manifest.write('''
[tasks]
manifest = "tools/tasks/Cargo.toml"
namespace = "sample"
[[tasks.commands]]
name = "mark"
description = "Write the requested marker"
[[tasks.commands.arguments]]
name = "value"
description = "Text to write"
required = true
[[tasks.commands.arguments]]
name = "fail"
kind = "flag"
description = "Report a test failure"
''')
        env = dict(os.environ, CARGO_NET_OFFLINE="true")
        for command in [["task","list"], ["help","--llm"]]:
            assert "sample:mark" in subprocess.check_output([binary,*command],cwd=root,text=True,env=env)
        marker = f"{name} with spaces $literal"
        cmd = [binary,"task","run","sample:mark","--value",marker]
        subprocess.run(cmd,cwd=root,env=env,check=True)
        assert (root/"result.txt").read_text() == marker
        assert subprocess.run(cmd+["--fail"],cwd=root,env=env).returncode == 19
    print("Both Rust task packages received their structured arguments and propagated status 19.")
