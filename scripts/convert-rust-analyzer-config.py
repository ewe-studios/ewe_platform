#!/usr/bin/env python3
"""
Convert rust-analyzer.toml to .zed/settings.json format.

Zed doesn't read rust-analyzer.toml directly, so this script converts
the TOML settings into the nested JSON structure that Zed expects.

Usage:
    python scripts/convert-rust-analyzer-config.py
"""

import tomllib
import json
from pathlib import Path


def convert_to_zed_format(config: dict) -> dict:
    """Convert flat TOML sections to nested Zed LSP format."""
    result = {}

    for section, values in config.items():
        if isinstance(values, dict):
            result[section] = convert_to_zed_format(values)
        else:
            result[section] = values

    return result


def main():
    root = Path(__file__).parent.parent
    toml_path = root / "rust-analyzer.toml"
    zed_dir = root / ".zed"
    zed_settings_path = zed_dir / "settings.json"

    # Read TOML config
    if not toml_path.exists():
        print(f"Error: {toml_path} not found")
        return 1

    with open(toml_path, "rb") as f:
        config = tomllib.load(f)

    # Build Zed settings structure
    zed_config = {
        "lsp": {
            "rust-analyzer": {
                "initialization_options": convert_to_zed_format(config)
            }
        }
    }

    # Ensure .zed directory exists
    zed_dir.mkdir(exist_ok=True)

    # Write JSON settings
    with open(zed_settings_path, "w") as f:
        json.dump(zed_config, f, indent=2)
        f.write("\n")

    print(f"✓ Converted {toml_path} -> {zed_settings_path}")
    return 0


if __name__ == "__main__":
    exit(main())
