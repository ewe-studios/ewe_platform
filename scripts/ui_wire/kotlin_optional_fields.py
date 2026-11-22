"""Keep existing Kotlin builder signatures when an optional wire field is added."""
import re
import sys
from pathlib import Path

DEFAULTS = {
    "ClientHello": {"subscriptionResume": "false", "requestTag": "0uL"},
    "ServerCaps": {"subscriptionResume": "false", "requestTag": "0uL"},
    "Control": {"replayExpiredOffset": "0"},
    "FrameMessage": {"subscriptionId": "0uL"},
    "FrameRequest": {"resumeOffset": "0"},
    "LiveMount": {"sharing": "ConnectionSharing.TransportDefault", "keepAlive": "false"},
}

for table, defaults in DEFAULTS.items():
    path = Path(sys.argv[1]) / "ewe" / "ui" / f"{table}.kt"
    source = path.read_text()
    match = re.search(rf"(?m)^        fun create{table}\(([^)]+)\) : Int \{{", source)
    if not match:
        raise ValueError(f"Missing Kotlin builder for {table}")
    fields = match[1].split(", ")
    names = [field.split(":")[0] for field in fields]
    if names[-len(defaults):] != list(defaults):
        raise ValueError(f"Unexpected Kotlin builder fields for {table}")
    original = ", ".join(fields[:-len(defaults)])
    values = ", ".join([*names[:-len(defaults)], *defaults.values()])
    overload = f"        fun create{table}({original}): Int = create{table}({values})\n"
    path.write_text(source[:match.start()] + overload + source[match.start():])
