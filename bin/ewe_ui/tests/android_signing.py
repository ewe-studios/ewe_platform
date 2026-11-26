"""Check streamed Android signature results through the installed application command."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
from zipfile import ZipFile

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory() as scratch:
    root = Path(scratch)
    subprocess.run(
        [binary, "new", "--name", "Signed", "--target", "android",
         "--app-id", "example.signed", "--host", "server"],
        cwd=root, check=True,
    )
    project = root / "Signed"
    android = project / "android"
    verifier = root / "report.py"
    verifier.write_text(
        "import os, sys\n"
        "sys.stdout.write('a' * 70000 + '\\n')\n"
        "sys.stdout.write(os.environ['VERIFICATION'] + '\\r\\n')\n"
        "sys.stdout.write('b' * 70000 + '\\n')\n"
        "sys.exit(int(os.environ['VERIFIER_EXIT']))\n"
    )
    if os.name == "nt":
        (android / "gradlew.bat").write_text("@exit /b 0\n")
        (root / "jarsigner.bat").write_text(f'@"{sys.executable}" "{verifier}"\n')
    else:
        wrapper = android / "gradlew"
        wrapper.write_text("#!/bin/sh\nexit 0\n")
        wrapper.chmod(0o755)
        tool = root / "jarsigner"
        tool.write_text("#!" + sys.executable + "\n" + verifier.read_text())
        tool.chmod(0o755)
    artifact = android / "app/build/outputs/bundle/release/app-release.aab"
    artifact.parent.mkdir(parents=True)
    with ZipFile(artifact, "w") as bundle:
        for abi in ["arm64-v8a", "x86_64"]:
            bundle.writestr(f"base/lib/{abi}/libfoundation_ui_native.so", b"fixture")
        bundle.writestr("META-INF/FIXTURE.RSA", b"fixture")
    for marker, code, success in [
        ("jar verified.", 0, True),
        ("warning: jar verified.", 0, False),
        ("jar verified. warning", 0, False),
        ("jar verified.", 1, False),
    ]:
        result = subprocess.run(
            [binary, "build", "--target", "android", "--release"],
            cwd=project, env=dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"],
                                  VERIFICATION=marker, VERIFIER_EXIT=str(code)),
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
        )
        assert (result.returncode == 0) == success, (marker, code, result.stderr)
print("Verified streaming signature output, complete matching lines, and child errors.")
