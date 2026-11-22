"""Check the two visible Android todo screens and their independent stores."""

import os
from pathlib import Path
import re
import subprocess
import struct
import sys
import time
import xml.etree.ElementTree as ET

adb, device, phase, *labels = sys.argv[1:]
app = "ewe.todo" if phase.startswith(("server", "peer-")) else "ewe.todo.local"
timeout = int(os.getenv("EWE_ANDROID_VIEW_TIMEOUT", "60"))


def shell(*args, command_timeout=10):
    return subprocess.check_output(
        [adb, "-s", device, "shell", *args], text=True, timeout=command_timeout
    )


def wait_for(*present, absent=(), condition=lambda _: True):
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
                if node.get("package") == app:
                    if node.get("content-desc"):
                        visible.setdefault(node.get("content-desc"), node)
                    if node.get("class") == "android.widget.EditText":
                        visible.setdefault("<editor>", node)
            if all(label in visible for label in present) and all(label not in visible for label in absent) and condition(visible):
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
    shell("input", "tap", str((left + right) // 2), str((top + bottom) // 2), command_timeout=timeout)


def create(title):
    tap("Add", "Todos")
    tap("Todo title", "New todo")
    shell("input", "text", title.replace(" ", "%s"), command_timeout=timeout)
    tap("Add", "New todo")
    wait_for(title, absent=("New todo",))


local_title = "Android offline todo"
server_title = "Android server todo"
if phase == "mathalley":
    output = Path(labels[0])
    output.mkdir(parents=True, exist_ok=True)
    shell("am", "start", "-S", "-n", app + "/.MainActivity", "--es", "gameSeed", "13")
    tap("Play Mathalley")
    wait_for("Back to Todos")

    def screenshot():
        return subprocess.check_output([adb, "-s", device, "exec-out", "screencap", "-p"])

    def read_scene():
        png = screenshot()
        width, height = struct.unpack(">II", png[16:24])
        pixels = subprocess.run(
            ["magick", "png:-", "-depth", "8", "rgb:-"],
            input=png, capture_output=True, check=True,
        ).stdout
        background = bytes.fromhex("1b1410")
        top = next((row for row in range(height)
                    if pixels[3 * width * row:3 * width * row + 3] == background), None)
        if top is None:
            return None
        scale = min(width / 390, (height - top) / 844)

        def position(x, y):
            return (round((width - 390 * scale) / 2 + x * scale),
                    round(top + (height - top - 844 * scale) / 2 + y * scale))

        x, y = position(70, 200)
        offset = 3 * (y * width + x)
        return png, position, pixels[offset:offset + 3].hex()

    def wait_for_color(color=None):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            result = read_scene()
            if result and (color is None or result[2] == color):
                return result[0], result[1]
            time.sleep(0.2)
        raise RuntimeError(f"The rendered game did not show {color}")

    def touch(position, x, y):
        px, py = position(x, y)
        shell("input", "tap", str(px), str(py))

    image, position = wait_for_color()
    (output / "android-title.png").write_bytes(image)
    touch(position, 195, 422)
    image, position = wait_for_color("274653")
    (output / "android-game.png").write_bytes(image)
    # Seed 13 gives these six carriage answers before Gumball.
    for index, x in enumerate([75, 195, 315, 315, 75, 315]):
        touch(position, x, 660)
        time.sleep(2.5 if index == 2 else 0.45)
    image, position = wait_for_color("c63f39")
    (output / "android-gumball.png").write_bytes(image)
    previous = shell("wm", "user-rotation").strip().split()
    shell("wm", "user-rotation", "lock", "1")
    try:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            image, position = wait_for_color("c63f39")
            width, height = struct.unpack(">II", image[16:24])
            if width > height:
                break
        else:
            raise RuntimeError("Gumball did not rotate to landscape")
        touch(position, 105, 690)
    finally:
        shell("wm", "user-rotation", *previous)
    tap("Back to Todos")
    wait_for("Play Mathalley")
elif phase == "offline":
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
elif phase == "peer-ready":
    wait_for("Todos", "Add", "0 open todos")
elif phase == "peer-draft":
    tap("Done")
    tap("Add", "Todos")
    wait_for("New todo", "Todo title", "Cancel")
elif phase == "peer-check-draft":
    wait_for("New todo", "Todo title", "Cancel")
    tap("Cancel")
    wait_for("Done", absent=(labels[0], "New todo"))
elif phase == "peer-check":
    title, state = labels
    if state == "absent":
        wait_for("Todos", absent=(title,))
    else:
        wait_for(title, state)
elif phase == "peer-show-all":
    tap("All")
elif phase == "peer-add":
    title = labels[0]
    tap("Add", "Todos")
    tap("<editor>", "New todo")
    def editor_ready(nodes):
        return nodes["<editor>"].get("focused") == "true" or (
            "mInputShown=true" in shell("dumpsys", "input_method")
        )
    wait_for("<editor>", condition=editor_ready)
    shell("input", "text", title.replace(" ", "%s"), command_timeout=timeout)
    wait_for(title, "New todo")
    tap("Add", "New todo")
    wait_for(title, absent=("New todo",))
elif phase == "peer-complete":
    tap("Complete", labels[0])
    wait_for(labels[0], "Reopen")
elif phase == "peer-reopen":
    tap("Reopen", labels[0])
    wait_for(labels[0], "Complete")
elif phase == "peer-delete":
    tap("Delete", labels[0])
    tap("Confirm", "Delete this todo?")
    wait_for("Todos", absent=(labels[0], "Delete this todo?"))
elif phase == "theme-review":
    output = Path(os.environ["EWE_THEME_CAPTURES"])
    output.mkdir(parents=True, exist_ok=True)
    old_timeout = shell("settings", "get", "system", "screen_off_timeout").strip()
    old_scale = shell("settings", "get", "system", "font_scale").strip()
    old_night = shell("cmd", "uimode", "night").strip().split()[-1]

    def capture(name):
        data = subprocess.check_output([adb, "-s", device, "exec-out", "screencap", "-p"])
        (output / (name + ".png")).write_bytes(data)

    try:
        shell("settings", "put", "system", "screen_off_timeout", str(timeout * 10000))
        for mode in ("light", "dark", "branded"):
            shell("input", "keyevent", "KEYCODE_WAKEUP")
            shell("wm", "dismiss-keyguard")
            shell("am", "force-stop", app)
            shell("cmd", "uimode", "night", "yes" if mode == "dark" else "no")
            shell("settings", "put", "system", "font_scale", "1.4" if mode == "branded" else "1.0")
            shell("am", "start", "-n", app + "/.MainActivity", "--ez", "branded", str(mode == "branded").lower())
            tap("Add", "Todos")
            tap("Todo title", "New todo")
            title = "Buy milk and collect the groceries for dinner"
            shell("input", "text", title.replace(" ", "%s"), command_timeout=timeout)
            controls = wait_for(title, "Cancel", "Add")
            cancel = [int(n) for n in re.findall(r"-?\d+", controls["Cancel"].get("bounds"))]
            submit = [int(n) for n in re.findall(r"-?\d+", controls["Add"].get("bounds"))]
            assert cancel[2] <= submit[0] and min(cancel[3], submit[3]) > max(cancel[1], submit[1]), (mode, cancel, submit)
            windows = shell("dumpsys", "input_method")
            assert "mInputShown=true" in windows, "The entry review needs the software keyboard"
            inset = re.search(r"type=ime frame=\[\d+,(\d+)\]\[\d+,\d+\][^\n]* visible=true", shell("dumpsys", "window"))
            assert inset and max(cancel[3], submit[3]) <= int(inset.group(1)), (mode, cancel, submit)
            print(f"{mode} entry controls passed. Cancel {cancel}, Add {submit}, keyboard top {inset.group(1)}.", flush=True)
            capture("android-" + mode + "-entry")
            tap("Cancel", title)
            wait_for("0 open todos", absent=(title, "New todo"))
            tap("Add", "Todos")
            tap("Todo title", "New todo")
            shell("input", "text", title.replace(" ", "%s"), command_timeout=timeout)
            if mode == "branded":
                tap("Add", title)
            else:
                shell("input", "keyevent", "KEYCODE_ENTER")
            wait_for(title, "Complete", absent=("New todo",))
            tap("Complete")
            wait_for(title, "Reopen")
            capture("android-" + mode + "-list")
            tap("Delete")
            controls = wait_for("Confirm", "Cancel", "Delete this todo?")
            cancel = [int(n) for n in re.findall(r"-?\d+", controls["Cancel"].get("bounds"))]
            confirm = [int(n) for n in re.findall(r"-?\d+", controls["Confirm"].get("bounds"))]
            assert min(cancel[3], confirm[3]) > max(cancel[1], confirm[1]), (mode, cancel, confirm)
            assert cancel[2] <= confirm[0] or confirm[2] <= cancel[0], (mode, cancel, confirm)
            capture("android-" + mode + "-confirmation")
            tap("Cancel")
            wait_for(title, absent=("Delete this todo?",))
            tap("Delete")
            tap("Confirm", "Delete this todo?")
            wait_for("0 open todos", absent=(title, "Delete this todo?"))
            print(f"{mode} confirmation controls passed. Cancel {cancel}, Confirm {confirm}.", flush=True)
    finally:
        if old_timeout == "null":
            shell("settings", "delete", "system", "screen_off_timeout")
        else:
            shell("settings", "put", "system", "screen_off_timeout", old_timeout)
        if old_scale == "null":
            shell("settings", "delete", "system", "font_scale")
        else:
            shell("settings", "put", "system", "font_scale", old_scale)
        shell("cmd", "uimode", "night", old_night)
else:
    raise SystemExit(f"Unsupported test phase {phase}")
