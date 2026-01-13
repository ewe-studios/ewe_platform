# Skills Directory

This directory contains documented skills that agents use to accomplish specific technical tasks.

## Purpose

Skills are detailed guides for:
- Using specific tools/libraries (e.g., Playwright, Docker, Kubernetes)
- Implementing specific patterns (e.g., authentication, caching)
- Performing specific operations (e.g., database migrations, API integration)
- Solving specific problems (e.g., performance optimization)

## Structure

Each skill is in its own directory with documentation and optionally supporting scripts:
```
.agents/skills/
├── README.md (this file)
├── 01-skill-name/
│   ├── skill.md (required)
│   ├── script.py (optional)
│   ├── helper.sh (optional)
│   └── examples/ (optional)
│       └── example-usage.*
├── 02-another-skill/
│   ├── skill.md
│   └── automation.js
└── [NN-skill-name]/
    ├── skill.md (required - always present)
    └── [optional scripts and code files]
```

### Supporting Files

Skills can include executable scripts and code:
- **Scripts**: `.sh`, `.py`, `.js`, `.ts`, `.rs`, etc.
- **Templates**: Reusable code to copy and adapt
- **Examples**: Working implementations
- **Configs**: YAML, JSON, TOML files

**Three Usage Types**:
1. **TEMPLATE**: Copy ALL skill files (templates + helpers) to project and customize
2. **EXECUTABLE**: Run scripts from `.agents/skills/` as external tools
3. **EDUCATIONAL**: Study examples, install external libraries (NPM/PyPI/etc.), implement fresh code

**CRITICAL**: Agents must NEVER `import/require/use` from `.agents/skills/` in project code. Skills directory is isolated - copy files to project first for TEMPLATE skills, run as external tools for EXECUTABLE skills, or implement fresh for EDUCATIONAL skills.

## Skill File Format

Every `skill.md` file must start with frontmatter:

```markdown
---
name: "Skill Name"
description: "Brief description of what this skill achieves"
approved: No  # Changed to Yes after user approval
created: YYYY-MM-DD
tools:
  - Tool1
  - Tool2
files:
  - script.py: "Description of what this script does and its Usage Type (EXECUTABLE/IMPORTABLE/TEMPLATE)"
  - helper.sh: "Description of helper script and its Usage Type"
---

# [Skill Content Follows]

## Attached Scripts and Code

### Script: script.py
**Purpose**: What this script does
**Language**: Python
**Usage**: How to use it
...

[Continue with skill documentation]
```

## Approval Process

1. **Agent identifies knowledge gap** during specification review
2. **Agent researches** using search tools, documentation, internet
3. **Agent creates skill document** if no alternative exists
4. **Main Agent reviews** for accuracy and necessity
5. **User approves** before skill can be used
6. **Frontmatter updated** to `approved: Yes`
7. **Agent proceeds** with implementation using approved skill

## When to Create Skills

**Create skills ONLY when**:
- ✅ Fundamental understanding is missing for required task
- ✅ No existing skill covers the need
- ✅ No alternative approach possible with existing knowledge
- ✅ User hasn't provided clear alternative instructions

**DO NOT create skills for**:
- ❌ Simple tasks agents should already know
- ❌ Basic programming concepts
- ❌ Tasks solvable with quick research
- ❌ Trivial operations

## Using Skills

### For Agents

**Before starting work**:
1. Scan `.agents/skills/` directory (read frontmatter only)
2. Identify relevant skills by name and description
3. Check if skills are approved (`approved: Yes`)
4. Read full skill content when ready to use

**During work**:
1. Apply skill patterns and examples
2. Update specification frontmatter with skill reference
3. Document deviations in learnings.md if needed

### For Users

**When new skill is created**:
1. Main Agent will report new skill creation
2. Review the skill document at `.agents/skills/[NN-name]/skill.md`
3. Approve or provide alternative approach
4. Skill is updated to `approved: Yes` upon approval

**Managing skills**:
- Review unapproved skills periodically
- Update skills with better practices as learned
- Remove obsolete or incorrect skills
- Add user-created skills for agent guidance

## Integration with Specifications

Specifications reference skills in frontmatter (one-way relationship):

```markdown
---
status: in-progress
skills:
  - playwright-web-interaction
  - jwt-authentication
tools:
  - TypeScript
  - Playwright
---
```

**Important**: Skills are independent documents and do NOT track which specifications use them. Only specifications reference skills, not the other way around.

## See Also

- **Rule 09**: Skills Identification and Creation (complete workflow)
- **Rule 04**: Agent Orchestration (when skills are identified)
- **Rule 06**: Specifications and Requirements (skill integration)

---

For complete details on the skills workflow, see `.agents/rules/09-skills-identification-and-creation.md`
