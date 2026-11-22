"""Keep unknown behaviour identifiers available to native renderer diagnostics."""
from pathlib import Path
import re
import sys

path = Path(sys.argv[1]) / "ops_generated.swift"
text = path.read_text()
for kind in ("BehaviourId", "MobileBehaviourId"):
    pattern = rf"  public var behaviour: ewe_ui_{kind} \{{[^\n]+\}}"
    match = re.search(pattern, text)
    if match is None:
        sys.exit(f"Missing generated Swift accessor for {kind}")
    source = match.group()
    raw = re.search(r"(_accessor\.readBuffer\(of: UInt16\.self, at: o\))", source)
    if raw is None:
        sys.exit(f"Unexpected generated Swift layout for {kind}")
    accessor = ("  public var rawBehaviour: UInt16 { let o = _accessor.offset(VTOFFSET.behaviour.v); "
                f"return o == 0 ? 0 : {raw.group(1)} }}")
    text = text.replace(source, source + "\n" + accessor)
path.write_text(text)

# Keep the schema identifier declarations stable across compiler versions.
frame = path.with_name("frame_generated.swift")
source = frame.read_text()
frame.write_text(re.sub(r'(public static var id: String \{ "EWF1" \}) +$', r'\1', source, flags=re.M))
