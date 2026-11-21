"""Keep absent and empty action input vectors distinct in the TypeScript object API."""
from pathlib import Path
import sys

path = Path(sys.argv[1]) / "ewe/ui/action-input.ts"
source = path.read_text()
changes = [
    ("public json: (number)[] = []", "public json: number[]|null = null", 1),
    ("this.bb!.createScalarList<number>(this.json.bind(this), this.jsonLength())",
     "this.jsonArray() === null ? null : Array.from(this.jsonArray()!)", 2),
    ("const json = ActionInput.createJsonVector(builder, this.json);",
     "const json = this.json === null ? 0 : ActionInput.createJsonVector(builder, this.json);", 1),
]
for old, new, expected in changes:
    if source.count(old) != expected:
        sys.exit("Review the generated TypeScript action input before applying the presence rule.")
    source = source.replace(old, new)
path.write_text(source)
