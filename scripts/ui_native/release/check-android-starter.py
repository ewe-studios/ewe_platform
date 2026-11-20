"""Check the starter's visible count before and after a real remote action."""
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

adb, device, url = sys.argv[1:]
timeout = int(os.environ.get("EWE_ANDROID_VIEW_TIMEOUT", "60"))


def shell(*args):
    return subprocess.check_output([adb, "-s", device, "shell", *args], text=True, timeout=timeout)


def wait_for(*labels):
    deadline = time.monotonic() + timeout
    visible = {}
    while time.monotonic() < deadline:
        try:
            shell("uiautomator", "dump", "/sdcard/ewe-starter.xml")
            tree = ET.fromstring(shell("cat", "/sdcard/ewe-starter.xml"))
            visible = {node.get("text"): node for node in tree.iter("node") if node.get("package") == "ewe.starter"}
            if all(label in visible for label in labels):
                return visible
        except (ET.ParseError, subprocess.CalledProcessError):
            pass
        time.sleep(1)
    raise RuntimeError(f"Missing starter labels {labels}. Visible labels: {list(visible)}")


wait_for("Open todos", "0", "Refresh")
subprocess.run(["node", Path(__file__).with_name("seed-starter.mjs"), url], check=True, timeout=timeout)
refresh = wait_for("0", "Refresh")["Refresh"]
left, top, right, bottom = map(int, re.findall(r"-?\d+", refresh.attrib["bounds"]))
shell("input", "tap", str((left + right) // 2), str((top + bottom) // 2))
wait_for("Open todos", "1", "Refresh")
print("Android starter changed the open todo count from 0 to 1 after Refresh.")
