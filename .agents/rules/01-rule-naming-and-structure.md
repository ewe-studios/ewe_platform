# Rule Naming and Structure Policy

## Purpose
This rule establishes the naming conventions and structural requirements for all rules in the `.agents/rules/` directory.

## Requirements

### File Format
- **MUST** be a Markdown file with `.md` extension
- **MUST** use dash (`-`) as the word separator
- **MUST** have a clear, descriptive name that articulates what the rule is about
- **MUST** be prefixed with a two-digit incrementing number (01, 02, 03, etc.)

### Naming Convention
```
NN-rule-name-describing-the-policy.md
```

Where `NN` is a two-digit number (01, 02, 03, etc.) that determines the loading order.

**Format Breakdown:**
- **Numerical Prefix**: Two-digit number (01-99) followed by a dash
- **Rule Name**: Descriptive name using dash separators
- **File Extension**: `.md`

**Loading Order:**
- Rules are loaded in numerical order based on their prefix
- Lower numbers are loaded first (01 loads before 02, which loads before 03, etc.)
- This ensures that foundational rules are loaded before rules that depend on them

**Examples of Good Names:**
- `01-rule-naming-and-structure.md`
- `02-rules-directory-policy.md`
- `03-commit-message-format.md`
- `04-code-review-requirements.md`
- `05-testing-standards.md`

**Examples of Bad Names:**
- `rule1.md` (incorrect prefix format, not descriptive)
- `1-rule-naming.md` (single digit instead of two digits)
- `01_rule_naming.md` (uses underscore instead of dash)
- `01-ruleNaming.md` (uses camelCase instead of dash)
- `01 my rule.md` (contains spaces)
- `rule-naming-and-structure.md` (missing numerical prefix)

### Directory Structure
- **MUST** use a flat structure - all rules in `.agents/rules/` directly
- **MUST NOT** create subdirectories within `.agents/rules/`
- **MUST NOT** nest rules in multiple levels of directories
- Each rule **MUST** be in its own separate file

**Correct Structure:**
```
.agents/
└── rules/
    ├── 01-rule-naming-and-structure.md
    ├── 02-rules-directory-policy.md
    ├── 03-commit-message-format.md
    └── 04-testing-standards.md
```

**Incorrect Structure:**
```
.agents/
└── rules/
    ├── coding/
    │   └── 01-style-guide.md       ❌ NO subdirectories
    ├── process/
    │   └── 02-review-process.md    ❌ NO subdirectories
    ├── all-rules.md                ❌ NO combining multiple rules
    └── rule-naming.md              ❌ Missing numerical prefix
```

## Rationale
- **Clarity**: Descriptive names make rules easy to find and understand
- **Consistency**: Uniform naming convention improves discoverability
- **Ordering**: Numerical prefixes ensure rules are loaded in a predictable, controlled order
- **Dependencies**: Loading order allows foundational rules to be applied before dependent rules
- **Simplicity**: Flat structure prevents organizational complexity
- **Accessibility**: All rules are immediately visible without navigation

## Enforcement
Any rule that violates these requirements must be:
1. Renamed to follow the naming convention
2. Moved to the flat `.agents/rules/` structure
3. Split into separate files if combining multiple rules

---
*Created: 2026-01-11*
