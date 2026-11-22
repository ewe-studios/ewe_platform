"""Point a copy of the starter project to a local Swift package."""
from pathlib import Path
import sys, json, re
candidate, package, project = map(Path, sys.argv[1:])
record = json.loads(candidate.read_text())
(package / "Package.swift").write_text(record["swift_manifest"])
text = project.read_text()
url = "https://github.com/" + record["repositories"]["repository"] + ".git"
pattern = r'repositoryURL = "' + re.escape(url) + r'";\s*requirement = \{[^}]*version = ' + re.escape(record["version"]) + r';[^}]*\};'
text, count = re.subn(pattern, 'relativePath = ' + json.dumps(str(package)) + ';', text)
if count != 1:
    raise SystemExit("Expected the starter to declare the public package at the release version")
project.write_text(text.replace("XCRemoteSwiftPackageReference", "XCLocalSwiftPackageReference"))
