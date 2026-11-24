"""Compare every installed command and its options with the agent guide."""
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory() as directory:
    def invoke(*words):
        return subprocess.check_output([binary, *words], cwd=directory, text=True,
                                       env=dict(os.environ, NO_COLOR="1"))

    guide = invoke("help", "--llm")
    sections = {}
    for section in re.split(r"(?=^Usage: )", guide, flags=re.MULTILINE)[1:]:
        header, _, body = section.partition("\n")
        sections[header] = body
    stack = [()]
    count = 0
    while stack:
        path = stack.pop()
        help_text = invoke(*path, "--help")
        usage = next(line for line in help_text.splitlines() if line.startswith("Usage: "))
        assert usage in sections, usage
        options = r"(?<!\w)--[a-z][a-z0-9-]*"
        assert set(re.findall(options, help_text[help_text.index(usage):])) <= set(re.findall(options, usage + sections[usage])), usage
        commands = re.search(r"^Commands:\n((?:  [^\n]*\n)+)", help_text, re.MULTILINE)
        if commands:
            for line in commands[1].splitlines():
                stack.append((*path, line.split()[0]))
        count += 1
    assert count == len(sections), (count, len(sections))
    print(f"All {count} installed commands and their options appear in the agent guide.")
