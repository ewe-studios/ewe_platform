"""Check the two visible Android todo screens and their independent stores."""

import os
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

adb, device, phase = sys.argv[1:]
app = "ewe.todo" if phase.startswith("server") else "ewe.todo.local"
timeout = int(os.getenv("EWE_ANDROID_VIEW_TIMEOUT", "60"))


def shell(*args, command_timeout=10):
    return subprocess.check_output(
        [adb, "-s", device, "shell", *args], text=True, timeout=command_timeout
    )


def wait_for(*present, absent=()):
    snapshot = "/sdcard/ewe-todo.xml"
    deadline = time.monotonic() + timeout
    visible = {}
    while time.monotonic() < deadline:
        try:
            report = shell("uiautomator", "dump", snapshot)
            if snapshot not in report:
                continue
            nodes = list(ET.fromstring(shell("cat", snapshot)).iter("node"))
            visible = {
                node.get("text"): node
                for node in nodes
                if node.get("package") == app and node.get("text")
            }
            for node in nodes:
                if node.get("package") == app and node.get("content-desc"):
                    visible.setdefault(node.get("content-desc"), node)
            if all(label in visible for label in present) and all(label not in visible for label in absent):
                return visible
        except (ET.ParseError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
            pass
        time.sleep(0.5)
    raise RuntimeError(
        f"{app} expected {present} and absent {absent}. Visible: {list(visible)}"
    )


def tap(text, *also_visible):
    node = wait_for(text, *also_visible)[text]
    left, top, right, bottom = map(int, re.findall(r"-?\d+", node.attrib["bounds"]))
    shell("input", "tap", str((left + right) // 2), str((top + bottom) // 2))


def create(title):
    tap("Add", "Todos")
    tap("Todo title", "New todo")
    shell("input", "text", title.replace(" ", "%s"))
    tap("Add", "New todo")
    wait_for(title, absent=("New todo",))


local_title = "Android offline todo"
server_title = "Android server todo"
if phase == "offline":
    package = shell("dumpsys", "package", app)
    assert "android.permission.INTERNET" not in package, "Offline app requests internet access"
    wait_for("Todos", "0 open todos", "Add", "All", "Open", "Done")
    create(local_title)
    wait_for(local_title, "1 open todo", "Complete")
    tap("Complete", local_title)
    wait_for("0 open todos", "Reopen")
    tap("Open")
    wait_for("0 open todos", absent=(local_title,))
    tap("Done")
    tap("Reopen", local_title)
    wait_for("1 open todo", absent=(local_title,))
    tap("Open")
    tap("Delete", local_title)
    tap("Confirm", "Delete this todo?")
    wait_for("0 open todos", absent=(local_title, "Delete this todo?"))
    create(local_title)
    wait_for("1 open todo")
    print("Offline app showed its initial view, added, completed, filtered, reopened and deleted a todo.")
elif phase == "server":
    wait_for("Todos", "Add", "0 open todos", absent=(local_title, server_title))
    create(server_title)
    wait_for(server_title, "1 open todo")
    tap("Complete", server_title)
    wait_for("0 open todos")
    tap("Done")
    tap("Reopen", server_title)
    wait_for("1 open todo")
    tap("Open")
    wait_for(server_title, "Complete", absent=(local_title,))
    print("Server app submitted and updated a todo in its own list.")
elif phase == "server-delete":
    tap("Delete", server_title)
    tap("Confirm", "Delete this todo?")
    wait_for("0 open todos", absent=(server_title, "Delete this todo?"))
    print("Recreated server screen received the task and displayed the count after deletion.")
elif phase == "isolation":
    wait_for(local_title, "1 open todo", absent=(server_title,))
    print("Offline app retained its own todo after the server app submitted a todo.")
else:
    raise SystemExit(f"Unsupported test phase {phase}")
