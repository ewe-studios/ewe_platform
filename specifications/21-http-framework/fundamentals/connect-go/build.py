#!/usr/bin/env python3
"""Build connect-go documentation HTML.

Usage:
    python3 build.py

Converts markdown/ to HTML in html/. Uses the shared build.py from
repo-expolorations if available, otherwise runs standalone.
"""

import os
import sys
from pathlib import Path

# Try to find the shared build.py
SCRIPT_DIR = Path(__file__).resolve().parent

# Search upward for repo-expolorations
search = SCRIPT_DIR
for _ in range(10):
    candidate = search / 'repo-expolorations' / 'build.py'
    if candidate.exists():
        shared = candidate
        break
    candidate = search.parent / 'repo-expolorations' / 'build.py'
    if candidate.exists():
        shared = candidate
        break
    search = search.parent
else:
    shared = None

if shared:
    # Import and run the shared builder
    import importlib.util
    spec = importlib.util.spec_from_file_location("build", shared)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    mod.build(str(SCRIPT_DIR))
else:
    print("Error: shared build.py not found. Expected at repo-expolorations/build.py")
    sys.exit(1)
