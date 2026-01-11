# Rule Naming and Structure Policy

## Purpose
This rule establishes the naming conventions and structural requirements for all rules in the `.agents/rules/` directory.

## Requirements

### File Format
- **MUST** be a Markdown file with `.md` extension
- **MUST** use dash (`-`) as the word separator
- **MUST** have a clear, descriptive name that articulates what the rule is about

### Naming Convention
```
rule-name-describing-the-policy.md
```

**Examples of Good Names:**
- `rule-naming-and-structure.md`
- `commit-message-format.md`
- `code-review-requirements.md`
- `testing-standards.md`

**Examples of Bad Names:**
- `rule1.md` (not descriptive)
- `rule_naming.md` (uses underscore instead of dash)
- `ruleNaming.md` (uses camelCase instead of dash)
- `my rule.md` (contains spaces)

### Directory Structure
- **MUST** use a flat structure - all rules in `.agents/rules/` directly
- **MUST NOT** create subdirectories within `.agents/rules/`
- **MUST NOT** nest rules in multiple levels of directories
- Each rule **MUST** be in its own separate file

**Correct Structure:**
```
.agents/
└── rules/
    ├── rule-naming-and-structure.md
    ├── rules-directory-policy.md
    ├── commit-message-format.md
    └── testing-standards.md
```

**Incorrect Structure:**
```
.agents/
└── rules/
    ├── coding/
    │   └── style-guide.md          ❌ NO subdirectories
    ├── process/
    │   └── review-process.md       ❌ NO subdirectories
    └── all-rules.md                ❌ NO combining multiple rules
```

## Rationale
- **Clarity**: Descriptive names make rules easy to find and understand
- **Consistency**: Uniform naming convention improves discoverability
- **Simplicity**: Flat structure prevents organizational complexity
- **Accessibility**: All rules are immediately visible without navigation

## Enforcement
Any rule that violates these requirements must be:
1. Renamed to follow the naming convention
2. Moved to the flat `.agents/rules/` structure
3. Split into separate files if combining multiple rules

---
*Created: 2026-01-11*
