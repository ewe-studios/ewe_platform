"""Check that Cargo includes every generator template in a clean source tree."""

from pathlib import Path
import subprocess
import sys

repo = Path(sys.argv[1]).resolve()
package = repo / "backends/foundation_ui_project"
members = subprocess.check_output(
    ["cargo", "package", "--offline", "--list", "--manifest-path", package / "Cargo.toml"],
    cwd=repo,
    text=True,
).splitlines()
templates = {
    path.relative_to(package).as_posix()
    for path in (package / "templates").rglob("*")
    if path.is_file()
}
missing = templates.difference(members)
assert templates and not missing, f"Missing template files: {sorted(missing)}"
print(f"Cargo includes all {len(templates)} template files.")
