"""Make every generated Rust union verifier check the opaque fallback."""

from pathlib import Path
import re
import sys


def main(generated: Path) -> None:
    fallback = re.compile(r"^( +)_ => Ok\(\(\)\),$", re.MULTILINE)
    union_count = 0
    for path in sorted(generated.rglob("*.rs")):
        source = path.read_text(encoding="utf-8")
        count = source.count(".visit_union::<")
        if not count:
            continue
        updated, replaced = fallback.subn(
            r"\1_ => crate::read::verify_opaque_union(v, pos),", source
        )
        if replaced != count:
            sys.exit(f"{path}: found {count} union verifiers and {replaced} opaque fallbacks")
        path.write_text(updated, encoding="utf-8")
        union_count += count
    if not union_count:
        sys.exit(f"{generated}: no generated union verifiers found")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
