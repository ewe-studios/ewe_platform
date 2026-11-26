"""Check the native command and the browser runtime use their intended wait implementations."""

from pathlib import Path
import subprocess

workspace = Path(__file__).resolve().parents[3]
version = subprocess.check_output(["rustc", "-vV"], cwd=workspace, text=True)
host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
for package, target, expected in [
    ("ewe_ui", host, False),
    ("foundation_wasm_ui", "wasm32-unknown-unknown", True),
]:
    graph = subprocess.check_output(
        ["cargo", "tree", "--locked", "--offline", "-p", package, "--target", target,
         "-e", "features", "-i", "foundation_nostd"],
        cwd=workspace, text=True,
    )
    uses_event_loop = 'foundation_nostd feature "js-event-loop"' in graph
    assert uses_event_loop == expected, (package, target, uses_event_loop)
print("Verified native thread parking and browser event loop feature selection.")
