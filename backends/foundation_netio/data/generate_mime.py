#!/usr/bin/env python3
"""Build the flat MIME table from a mime-db download.

This script reads `db.json` from jshttp/mime-db and writes one JSON object.
Each key is a lower-case file extension without the dot. Each value is one
bare media type, never a parameter.

The rule for an extension with several candidate types (rule R7):

1. Prefer the candidate whose `source` is "iana".
2. Then prefer a candidate with no `source` key.
3. Then prefer a candidate whose `source` is "apache".
4. Then prefer any other candidate.
5. Inside one rank, prefer the alphabetically first media type.

The writer sorts the extensions and uses two spaces of indent with one pair
per line, so a new mime-db version gives a diff of data rows only.

Usage:
    python3 -I generate_mime.py db.json mime.json
"""

import json
import sys

# The rank of one `source` value. A smaller rank wins.
RANK_IANA = 0
RANK_UNLISTED = 1
RANK_APACHE = 2
RANK_OTHER = 3


def source_rank(source):
    """Return the rank of one `source` value."""
    if source == "iana":
        return RANK_IANA
    if source is None:
        return RANK_UNLISTED
    if source == "apache":
        return RANK_APACHE
    return RANK_OTHER


def build(db):
    """Return the flat table for one parsed mime-db object."""
    # extension -> list of (rank, media type)
    candidates = {}
    for media_type, entry in db.items():
        source = entry.get("source")
        for extension in entry.get("extensions", ()):
            key = extension.lower()
            candidates.setdefault(key, []).append((source_rank(source), media_type))

    # The tuple order breaks a tie inside one rank by media type.
    chosen = {key: min(options)[1] for key, options in candidates.items()}
    ambiguous = sum(1 for options in candidates.values() if len(options) > 1)
    return chosen, ambiguous


def write(table, path):
    """Write the table with sorted keys and two spaces of indent."""
    ordered = {key: table[key] for key in sorted(table)}
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(ordered, handle, indent=2, ensure_ascii=False)
        handle.write("\n")


def main(argv):
    if len(argv) != 3:
        print("usage: python3 -I generate_mime.py db.json mime.json", file=sys.stderr)
        return 2

    with open(argv[1], encoding="utf-8") as handle:
        db = json.load(handle)

    table, ambiguous = build(db)
    write(table, argv[2])
    print(f"{len(db)} media types")
    print(f"{len(table)} extensions")
    print(f"{ambiguous} ambiguous extensions")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
