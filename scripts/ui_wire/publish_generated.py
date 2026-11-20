"""Publish a complete set of generated files. Restore old files if a move fails."""

from contextlib import contextmanager
from hashlib import sha256
import os
from pathlib import Path
import subprocess
import sys
import tempfile


@contextmanager
def publication_lock(output: Path):
    if os.name == "nt":
        import msvcrt

        # Windows does not support locking an open directory.
        identity = os.path.normcase(str(output.resolve()))
        lock_name = f"ewe-wire-{sha256(identity.encode()).hexdigest()}.lock"
        with (Path(tempfile.gettempdir()) / lock_name).open("a+b") as lock:
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
            try:
                yield
            finally:
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
    else:
        import fcntl

        lock = os.open(output, os.O_RDONLY)
        try:
            fcntl.flock(lock, fcntl.LOCK_EX)
            yield
        finally:
            os.close(lock)


def publish(stage: Path, output: Path, asset: Path) -> None:
    items = [
        (stage / "src/generated", output / "src/generated"),
        (stage / "generated", output / "generated"),
        (stage / "foundation-ui-wire.js", asset),
    ]
    with publication_lock(output):
        moves = []
        try:
            for index, (source, destination) in enumerate(items):
                destination.parent.mkdir(parents=True, exist_ok=True)
                backup = None
                if destination.exists():
                    result = subprocess.run(
                        ["diff", "-qr", source, destination], stdout=subprocess.DEVNULL
                    )
                    if result.returncode == 0:
                        continue
                    if result.returncode != 1:
                        result.check_returncode()
                    backup = stage / f"previous-{index}"
                    destination.replace(backup)
                moves.append((source, destination, backup))
                source.replace(destination)
        except BaseException:
            for source, destination, backup in reversed(moves):
                if not source.exists():
                    destination.replace(source)
                if backup is not None:
                    backup.replace(destination)
            raise


if __name__ == "__main__":
    publish(*(Path(arg) for arg in sys.argv[1:]))
