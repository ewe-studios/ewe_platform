#!/usr/bin/env python3
"""
Script to add tracing spans to Rust functions that don't have #[tracing::instrument] markers.
This version properly handles impl blocks and trait implementations.
"""

import os
import re
from pathlib import Path

class RustFunctionTracer:
    def __init__(self):
        self.instrument_pattern = re.compile(r'#\s*\[\s*(tracing::)?instrument\b')
        self.test_pattern = re.compile(r'#\s*\[\s*(test|cfg\s*\(\s*test\s*\))\s*\]')
        self.derive_pattern = re.compile(r'#\s*\[\s*derive\s*\(')
        self.allow_pattern = re.compile(r'#\s*\[\s*allow\s*\(')
        self.must_use_pattern = re.compile(r'#\s*\[\s*must_use\s*\]')
        self.doc_pattern = re.compile(r'#\s*\[\s*doc\s*=')
        self.func_pattern = re.compile(r'^(?P<indent>\s*)(?P<async>async\s+)?(?P<unsafety>unsafe\s+)?(?P<extern>extern\s+"[^"]+"\s+)?fn\s+(?P<func_name>\w+)\s*(?P<generics><[^>]*>)?\s*\(')

    def is_attribute_line(self, line):
        """Check if a line is an attribute (starts with #[)."""
        stripped = line.strip()
        return (stripped.startswith('#[') or
                self.derive_pattern.search(stripped) or
                self.allow_pattern.search(stripped) or
                self.must_use_pattern.search(stripped) or
                self.doc_pattern.search(stripped))

    def get_attributes_before(self, lines, line_idx):
        """Get all attribute lines before the given line."""
        attrs = []
        for i in range(line_idx - 1, max(line_idx - 15, -1), -1):
            if self.is_attribute_line(lines[i]):
                attrs.append((i, lines[i]))
            elif lines[i].strip() and not lines[i].strip().startswith('//'):
                break  # Hit a non-attribute line
        return attrs

    def has_instrument_attr(self, lines, line_idx):
        """Check if function has instrument attribute."""
        attrs = self.get_attributes_before(lines, line_idx)
        for _, attr_line in attrs:
            if self.instrument_pattern.search(attr_line):
                return True
        return False

    def has_test_attr(self, lines, line_idx):
        """Check if function has test attribute."""
        attrs = self.get_attributes_before(lines, line_idx)
        for _, attr_line in attrs:
            if self.test_pattern.search(attr_line):
                return True
        return False

    def find_function_body(self, lines, start_idx):
        """Find the function body start (opening brace) and return (body_start_idx, is_trait_method)."""
        brace_count = 0
        paren_count = 0
        in_params = False
        found_open_brace = False
        body_start = None

        # First, find the opening parenthesis if not on the same line
        line = lines[start_idx]
        if '(' in line:
            in_params = True
            paren_count = line.count('(') - line.count(')')
            if paren_count == 0:
                in_params = False

        for i in range(start_idx, len(lines)):
            line = lines[i]
            j = 0
            while j < len(line):
                char = line[j]

                # Handle string literals and comments
                if j < len(line) - 1 and line[j:j+2] == '//':
                    break  # Skip rest of line (comment)
                if j < len(line) - 1 and line[j:j+2] == '/*':
                    # Skip block comment
                    end = line.find('*/', j + 2)
                    if end != -1:
                        j = end + 1
                        continue

                if char == '(' :
                    paren_count += 1
                    in_params = True
                elif char == ')':
                    paren_count -= 1
                    if paren_count == 0:
                        in_params = False
                elif char == '{':
                    if not in_params:
                        brace_count += 1
                        if not found_open_brace:
                            found_open_brace = True
                            body_start = i
                elif char == '}':
                    if found_open_brace:
                        brace_count -= 1
                        if brace_count == 0:
                            return body_start, False  # Function end
                elif char == ';' and found_open_brace == False and paren_count == 0:
                    # Trait method or function signature without body
                    return None, True

            if found_open_brace and brace_count == 0:
                return body_start, False

        return body_start, False

    def add_span_to_file(self, filepath):
        """Add tracing spans to a file. Returns list of modified function names."""
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()
        except Exception as e:
            print(f"  Error reading {filepath}: {e}")
            return []

        lines = content.splitlines()
        modified = False
        funcs_modified = []
        skip_until_brace_close = 0  # Track macro blocks to skip

        i = 0
        while i < len(lines):
            line = lines[i]

            # Skip macro_rules! definitions
            if 'macro_rules!' in line:
                brace_count = 0
                started = False
                while i < len(lines):
                    for c in lines[i]:
                        if c == '{':
                            brace_count += 1
                            started = True
                        elif c == '}':
                            brace_count -= 1
                    if started and brace_count == 0:
                        break
                    i += 1
                i += 1
                continue

            # Check for #[test] or #[cfg(test)] at current line
            if self.test_pattern.search(line):
                i += 1
                continue

            # Check if this line defines a function
            match = self.func_pattern.match(line)
            if match:
                func_name = match.group('func_name')
                indent = match.group('indent')

                # Skip if already has instrument attribute
                if self.has_instrument_attr(lines, i):
                    i += 1
                    continue

                # Skip test functions
                if self.has_test_attr(lines, i):
                    i += 1
                    continue

                # Find function body
                body_start, is_trait_method = self.find_function_body(lines, i)

                if is_trait_method or body_start is None:
                    i += 1
                    continue

                # Find insertion point (first non-empty line after opening brace)
                insert_idx = body_start + 1
                while insert_idx < len(lines):
                    stripped = lines[insert_idx].strip()
                    if stripped and stripped != '}':
                        break
                    if stripped == '}':
                        insert_idx = None
                        break
                    insert_idx += 1

                if insert_idx is None or insert_idx >= len(lines):
                    i += 1
                    continue

                # Check if span already exists
                if 'let _span = tracing::span!' in lines[insert_idx]:
                    i += 1
                    continue

                # Create the tracing span line
                span_line = f'{indent}    let _span = tracing::span!(tracing::Level::TRACE, "{func_name}").entered();'

                # Insert the span line
                lines.insert(insert_idx, span_line)
                modified = True
                funcs_modified.append(func_name)

                # Skip ahead
                i = insert_idx + 2
                continue

            i += 1

        if modified:
            try:
                with open(filepath, 'w', encoding='utf-8') as f:
                    f.write('\n'.join(lines))
                print(f"Modified: {filepath}")
                if funcs_modified:
                    print(f"  Added spans to: {', '.join(funcs_modified[:10])}{'...' if len(funcs_modified) > 10 else ''}")
            except Exception as e:
                print(f"  Error writing {filepath}: {e}")
                return []

        return funcs_modified


def main():
    base_dir = Path('/home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src')
    tracer = RustFunctionTracer()

    rs_files = list(base_dir.rglob('*.rs'))
    print(f"Found {len(rs_files)} Rust files to process")

    total_modifications = 0
    skipped_files = []

    for filepath in sorted(rs_files):
        filepath_str = str(filepath)

        # Skip test files
        if '/tests/' in filepath_str or filepath.name.endswith('_test.rs'):
            skipped_files.append((filepath, "test file"))
            continue

        # Skip macro directories
        if '/macros/' in filepath_str:
            skipped_files.append((filepath, "macros directory"))
            continue

        funcs = tracer.add_span_to_file(filepath)
        total_modifications += len(funcs)

    print(f"\n=== Summary ===")
    print(f"Total functions modified: {total_modifications}")
    print(f"Files skipped: {len(skipped_files)}")

if __name__ == '__main__':
    main()
