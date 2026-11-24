"""Drive the installed wizard with a pseudo-terminal and compare its output to flags."""
import fcntl
import os
import pathlib
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

binary = str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    def execute(folder, actions, expected):
        folder.mkdir()
        master, slave = pty.openpty()
        original = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 90, 0, 0))
        proc = subprocess.Popen([binary, "new"], cwd=folder, stdin=slave, stdout=slave, stderr=slave)
        data = b""
        try:
            for message, keys in actions:
                end = time.monotonic() + 8
                while message not in data:
                    timeout = end - time.monotonic()
                    assert timeout > 0 and select.select([master], [], [], timeout)[0], (message, data[-600:])
                    data += os.read(master, 8192)
                data = b""
                if keys is None:
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 96, 0, 0))
                    proc.send_signal(signal.SIGWINCH)
                else:
                    os.write(master, keys)
            assert proc.wait(timeout=8) == expected
            assert termios.tcgetattr(slave) == original
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            os.close(slave)
            os.close(master)

    both = root / "interactive"
    execute(both, [
        (b"Application name", b"1\r"), (b"Invalid --name", b"\x7fFieldNotes\r"),
        (b"Desktop   Unavailable", None), (b"Desktop   Unavailable", b"\x1b[B \r"),
        (b"Mobile application identifier", b"dev.example.fieldnotes\r"), (b"Mobile host", b"\r"),
        (b"Create ./FieldNotes", b"\r")
    ], 0)
    direct = root / "direct"
    subprocess.run([binary, "new", "--name", "FieldNotes", "--target", "web", "--target", "android",
                    "--app-id", "dev.example.fieldnotes", "--output", str(direct)], check=True)
    def files(folder):
        return {p.relative_to(folder): p.read_bytes() for p in folder.rglob("*") if p.is_file()}
    assert files(both) == files(direct)
    cancelled = root / "cancelled"
    execute(cancelled, [(b"Application name", b"Cancelled\r"), (b"Desktop   Unavailable", b"\x1b")], 130)
    assert not list(cancelled.iterdir())
    print("Wizard and flags produced identical files. Resize, correction, cancellation and terminal restoration passed.")
