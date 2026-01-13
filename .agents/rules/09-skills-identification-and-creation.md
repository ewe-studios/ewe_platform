# Skills Identification and Creation

## Purpose
This rule establishes a structured approach for agents to identify knowledge gaps, document required skills, and obtain user approval before using them. This ensures agents have proper guidance for complex tasks while maintaining user oversight and control.

## Core Principles

### 1. Skills as Knowledge Assets
Skills are documented know-how for accomplishing specific technical tasks. They capture:
- How to use specific tools/libraries (e.g., Playwright, Docker, Kubernetes)
- How to implement specific patterns (e.g., authentication flows, caching strategies)
- How to perform specific operations (e.g., database migrations, API integration)
- How to solve specific problems (e.g., performance optimization, security hardening)

### 2. User Approval Required
No skill can be used until the user approves it. This ensures:
- User maintains control over approaches and methodologies
- User can provide feedback or alternative solutions
- Agent doesn't proceed with incorrect or suboptimal approaches
- Critical skills are validated before implementation

### 3. Last Resort Only
Agents **MUST NOT** create skills casually. Skills are created ONLY when:
- ✅ Fundamental understanding is missing for a required task
- ✅ No existing skill covers the need
- ✅ No alternative approach is possible with existing knowledge
- ✅ User hasn't provided clear instructions for alternatives
- ❌ NOT for simple tasks agents should already know
- ❌ NOT for basic programming concepts
- ❌ NOT to avoid thinking or problem-solving

## Skills Directory Structure

### Location
```
.agents/skills/
├── 01-playwright-web-interaction/
│   ├── skill.md (required - canonical documentation)
│   ├── learnings.md (optional - practical learnings from usage)
│   ├── browser-automation.js (executable script with arguments)
│   ├── scraper-module.ts (importable module with functions)
│   └── examples/
│       ├── login-flow.js (pattern example)
│       └── data-extraction.js (pattern example)
├── 02-kubernetes-deployment/
│   ├── skill.md (required)
│   ├── learnings.md (optional)
│   ├── deploy.sh
│   ├── rollback.sh
│   └── configs/
│       ├── deployment.yaml
│       └── service.yaml
├── 03-jwt-authentication/
│   ├── skill.md (required)
│   ├── learnings.md (optional)
│   ├── token-generator.py
│   ├── validator.rs
│   └── middleware.ts
└── [NN-skill-name]/
    ├── skill.md (required - canonical)
    ├── learnings.md (optional - practical usage insights)
    ├── script1.py (optional)
    ├── script2.sh (optional)
    ├── helper.rs (optional)
    └── examples/ (optional)
        └── example-usage.*
```

### Naming Convention
- Skills use numeric prefixes: `01-`, `02-`, `03-`, etc.
- Descriptive kebab-case names: `playwright-web-interaction`
- Format: `[NN-descriptive-skill-name]/skill.md` (required)
- Supporting files: Any relevant code, scripts, configs in same directory

### Supporting Files
Skills can include executable code and scripts to help agents:

**Common file types**:
- **skill.md**: (required) Canonical documentation - all formal skill knowledge
- **learnings.md**: (optional) Practical learnings from actual usage
- **Scripts**: `.sh`, `.bash`, `.py`, `.js`, `.ts`, `.rs`, etc.
- **Code templates**: Starting-point code designed to be copied and customized
- **Helper modules**: Supporting code to be copied along with templates
- **Configurations**: YAML, JSON, TOML configs
- **Examples**: Working examples showing patterns

**Three Usage Types for Skills**:

1. **TEMPLATE SKILLS** (copy ALL files to project and customize):
   - skill.md explicitly states "Copy and adapt" or "Template-based"
   - Agents copy ALL skill files (templates, helpers, configs) to project
   - Agents customize the copied files for specific use case
   - Original files in `.agents/skills/` remain untouched
   - Example: API client template with helper functions - copy everything
   - ✅ Agents modify the COPIED files in project directory
   - ❌ **NEVER import from `.agents/skills/` into project code**
   - ❌ Original skill files stay in `.agents/skills/` untouched

2. **EXECUTABLE SKILLS** (run as external tools):
   - skill.md provides scripts designed to be executed
   - Agents run scripts as external commands and consume output
   - Scripts act as tools/utilities that return results
   - Example: `node .agents/skills/01-scraper/scraper.js --url https://example.com`
   - ✅ Execute scripts, capture output, use results
   - ❌ Never modify script code
   - ❌ Scripts are not part of project code - they're external tools

3. **EDUCATIONAL SKILLS** (learn pattern, implement in project):
   - skill.md teaches concepts and shows example code
   - skill.md references external libraries to install (NPM, PyPI, Cargo, etc.)
   - Agents learn the pattern and implement fresh code in project
   - Example: "Install `jsonwebtoken` from NPM, then implement JWT validation"
   - ✅ Install external library as dependency
   - ✅ Study skill examples
   - ✅ Write fresh implementation in project following the pattern
   - ❌ **NEVER import from `.agents/skills/` directory**
   - ❌ skill examples are for learning, not for importing

**CRITICAL RULE - Skills Directory Isolation**:
- ❌ **NEVER `import/require/use` from `.agents/skills/` in project code**
- ❌ `.agents/skills/` is NOT a code library
- ❌ `.agents/skills/` is NOT part of the project
- ✅ `.agents/skills/` is a knowledge base and tool collection
- ✅ Template files are COPIED to project then modified
- ✅ Executable scripts are RUN as external tools
- ✅ Educational content teaches you to use external libraries

**Purpose**:
- Provide ready-to-use templates that can be copied and customized
- Provide external tool scripts that return useful results
- Teach patterns and demonstrate external library usage
- Keep skills isolated and safe from project changes
- Demonstrate best practices with working examples
- Capture practical insights from real usage

**Critical Rules**:
- ✅ Agents MUST check **Usage Type** in skill.md for each skill
- ✅ Copy ALL files when skill is marked as TEMPLATE (including helpers)
- ✅ Execute scripts and consume output for EXECUTABLE skills
- ✅ Learn patterns and implement fresh for EDUCATIONAL skills
- ✅ Install external dependencies as specified (NPM, PyPI, etc.)
- ❌ NEVER modify original files in `.agents/skills/` directory
- ❌ NEVER import/require code from `.agents/skills/` into project
- ❌ Skills directory must remain completely isolated from project code

**If uncertain about usage**:
1. Check skill.md for explicit usage instructions
2. Look for phrases: "copy and adapt", "execute script", "install library X"
3. If still ambiguous, ask Main Agent for clarification

### skill.md vs learnings.md

**skill.md** (Canonical Documentation):
- ✅ **Primary source of truth** for skill knowledge
- ✅ Formal, structured documentation
- ✅ Research-based, validated information
- ✅ Updated through formal approval process
- ✅ Always read when skill is selected for use
- ✅ Contains: Overview, prerequisites, concepts, step-by-step guides, patterns, pitfalls

**learnings.md** (Practical Insights):
- ✅ **Practical knowledge from actual usage**
- ✅ Discoveries made while applying the skill
- ✅ Edge cases encountered in production
- ✅ Gotchas not covered in official docs
- ✅ Real-world adaptations that worked
- ✅ Performance tips from experience
- ✅ Updated by agents after using skill
- ✅ **Only read when skill is actively being used** (not during frontmatter scan)
- ✅ Contains: Quick tips, common issues, solutions that worked, anti-patterns discovered

**Key Difference**:
- **skill.md** = What you need to know BEFORE using the skill
- **learnings.md** = What you learn AFTER using the skill

**When agents read each**:
```
Frontmatter scan (discovery):
  ✅ Read: skill.md frontmatter only
  ❌ Skip: learnings.md (not needed yet)

Skill selected for use:
  ✅ Read: skill.md (complete)
  ✅ Read: learnings.md (if exists)
  ✅ Read: relevant scripts

During implementation:
  ✅ Reference: skill.md, learnings.md, scripts as needed

After implementation:
  ✅ Update: learnings.md with new insights
  ✅ Update: skill.md if fundamental changes needed (requires approval)
```

## Skill File Format

### Frontmatter (Required)
Every `skill.md` file **MUST** start with frontmatter:

```markdown
---
name: "Playwright Web Interaction"
description: "Guide for using Playwright to automate browser interactions, scraping, and testing web applications"
approved: No
created: 2026-01-13
tools:
  - Playwright
  - TypeScript
  - Browser Automation
files:
  - browser-automation.js: "Executable script for browser automation (accepts arguments)"
  - scraper-module.ts: "TypeScript module with reusable scraping functions"
  - examples/login-flow.js: "Example pattern: Automated login flow"
  - examples/data-extraction.js: "Example pattern: Extract data from pages"
---
```

**Frontmatter Fields**:
- `name`: Clear, concise skill name (2-6 words)
- `description`: 1-2 sentence summary of what skill achieves and when to use it
- `approved`: `Yes` or `No` (defaults to `No` until user approves)
- `created`: Date skill was created (YYYY-MM-DD)
- `tools`: List of tools/technologies this skill covers
- `files`: Dictionary of attached files with brief descriptions (optional)

### Skill Content Structure

After frontmatter, the skill document should contain:

```markdown
# [Skill Name]

## Overview
Brief overview of what this skill is about (2-3 paragraphs).

## When to Use This Skill
- List specific scenarios where this skill applies
- Be clear about scope and limitations
- Include use cases

## Prerequisites
- Knowledge required before using this skill
- Dependencies that must be installed
- Environment setup needed

## Attached Scripts and Code

**IMPORTANT**: Clearly specify the skill's usage type and how agents should use the files.

### Example 1: TEMPLATE Skill (Copy All Files and Customize)

**Skill Usage Type**: TEMPLATE - Copy all files to project and customize

### Template: api-client.ts
**Purpose**: Main API client implementation template
**Language**: TypeScript
**Usage**: COPY to your project and customize for your API

**Instructions**:
1. Copy this file to your project: `cp api-client.ts src/clients/your-api-client.ts`
2. Customize the base URL for your specific API
3. Add your API-specific endpoints
4. Modify types to match your API schema

### Helper: http-helpers.ts
**Purpose**: HTTP utility functions
**Language**: TypeScript
**Usage**: COPY along with api-client.ts (part of template)

**Instructions**:
1. Copy to your project: `cp http-helpers.ts src/clients/http-helpers.ts`
2. Customize error handling if needed
3. Adapt retry logic for your requirements

**How Agent Uses This TEMPLATE Skill**:
```bash
# Step 1: Copy ALL files to project (template + helpers)
cp .agents/skills/05-rest-api/api-client.ts ./src/clients/product-api-client.ts
cp .agents/skills/05-rest-api/http-helpers.ts ./src/clients/http-helpers.ts

# Step 2: Modify the COPIED files in your project
# - Customize baseURL
# - Add product-specific endpoints
# - Update types for product API

# Step 3: Use within project (import from project, NOT from .agents/skills/)
# In src/main.ts:
import { ProductApiClient } from './clients/product-api-client';
import { handleHttpError } from './clients/http-helpers';

const client = new ProductApiClient();
```

**CRITICAL**:
- ❌ NEVER `import from './.agents/skills/...'` in project code
- ✅ Copy files to project, then import from project location
- ✅ Original files in `.agents/skills/` remain untouched

---

### Example 2: EXECUTABLE Skill (Run as External Tool)

**Skill Usage Type**: EXECUTABLE - Run scripts as external tools

### Script: scraper.js
**Purpose**: Web scraping utility script
**Language**: JavaScript
**Usage**: EXECUTE as external command with arguments

**Usage**:
```bash
node scraper.js --url <URL> --selector <CSS_SELECTOR> [--output <FILE>]
```

**Arguments**:
- `--url`: Target URL to scrape (required)
- `--selector`: CSS selector for data extraction (required)
- `--output`: Output file path (optional, defaults to stdout)

**Return Value**:
- Exits with 0 on success, 1 on error
- Outputs JSON data to stdout or specified file

**How Agent Uses This EXECUTABLE Skill**:
```bash
# Agent executes the script from .agents/skills/ as external tool
node .agents/skills/01-scraper/scraper.js \
  --url "https://example.com/products" \
  --selector ".product-item" \
  --output ./data/products.json

# Then reads the output in project code
# In src/data-loader.ts:
import fs from 'fs';
const products = JSON.parse(fs.readFileSync('./data/products.json', 'utf8'));
```

**CRITICAL**:
- ✅ Execute script from `.agents/skills/` location
- ✅ Script stays external to project code
- ❌ Never copy or modify the script
- ❌ Script is a tool, not part of project codebase

---

### Example 3: EDUCATIONAL Skill (Learn Pattern, Implement Fresh)

**Skill Usage Type**: EDUCATIONAL - Learn pattern and implement in your project

**External Dependencies**:
- Install: `npm install jsonwebtoken`
- Install: `npm install @types/jsonwebtoken --save-dev`

### Example: jwt-auth-example.ts
**Purpose**: Demonstrates JWT authentication pattern
**Language**: TypeScript
**Usage**: STUDY this example, then IMPLEMENT fresh code in your project using `jsonwebtoken` library

**Example code (for learning)**:
```typescript
import jwt from 'jsonwebtoken';

// Example pattern - implement similar logic in your project
export function generateToken(userId: string, secret: string): string {
  return jwt.sign({ userId }, secret, { expiresIn: '24h' });
}

export function validateToken(token: string, secret: string): { userId: string } | null {
  try {
    const decoded = jwt.verify(token, secret) as { userId: string };
    return decoded;
  } catch {
    return null;
  }
}
```

**How Agent Uses This EDUCATIONAL Skill**:
```bash
# Step 1: Install external library as dependency
npm install jsonwebtoken
npm install @types/jsonwebtoken --save-dev

# Step 2: Study the example pattern in skill
# Read: .agents/skills/03-jwt-auth/jwt-auth-example.ts

# Step 3: Implement FRESH code in project following the pattern
# In src/auth/jwt-service.ts:
import jwt from 'jsonwebtoken';  // From NPM, NOT from .agents/skills/

export class JwtService {
  constructor(private secret: string) {}

  // Implement pattern learned from skill
  generateToken(userId: string): string {
    return jwt.sign({ userId }, this.secret, { expiresIn: '24h' });
  }

  validateToken(token: string): { userId: string } | null {
    try {
      return jwt.verify(token, this.secret) as { userId: string };
    } catch {
      return null;
    }
  }
}
```

**CRITICAL**:
- ✅ Install external library (`jsonwebtoken` from NPM)
- ✅ Study skill example to learn the pattern
- ✅ Write fresh implementation in project
- ❌ **NEVER import from `.agents/skills/` directory**
- ❌ Example code is for learning, not for importing

[Continue for each script/file...]

## Core Concepts
Key concepts needed to understand this skill:
- Concept 1: Explanation
- Concept 2: Explanation
- Concept 3: Explanation

## Step-by-Step Guide

### Step 1: [First Step Name]
Detailed explanation with code examples.

**Using attached script**:
```bash
# Execute script with arguments for your specific use case
node browser-automation.js --url https://example.com --selector ".data-class" --timeout 60000
```

**Or import and call functions**:
```javascript
// Import from browser-automation.js
import { launchBrowser, extractData } from './browser-automation.js';

// Call with your parameters
const browser = await launchBrowser({ headless: true });
const data = await extractData(page, '.data-class');
```

### Step 2: [Second Step Name]
Detailed explanation with code examples.

[Continue for all steps...]

## Common Patterns
Frequently used patterns when applying this skill:
- Pattern 1: When and how to use (reference scripts if applicable)
- Pattern 2: When and how to use (reference scripts if applicable)

## Pitfalls to Avoid
Common mistakes and how to avoid them:
- Pitfall 1: What to avoid and why
- Pitfall 2: What to avoid and why

## Examples
Real-world examples demonstrating the skill:

### Example 1: [Scenario - User Login Flow]
**Using**: `examples/login-flow.js`
```javascript
// See examples/login-flow.js for complete implementation
// This example shows the pattern for login automation
await page.goto('https://example.com/login');
await page.fill('#username', 'user@example.com');
await page.fill('#password', 'password123');
await page.click('#login-button');
await page.waitForSelector('.dashboard');
```

**How agents use this**:
1. Read the example to understand the login pattern
2. Implement similar pattern in your code with your specific selectors
3. **Do NOT modify the example file** - implement the pattern in your own code
4. If the example needs improvement, request approval to update it

### Example 2: [Scenario - Data Extraction]
**Using**: `examples/data-extraction.js`
```javascript
// See examples/data-extraction.js for complete implementation
// This example demonstrates data extraction patterns
const data = await extractData(page, '.product-list .item', {
  fields: ['name', 'price', 'availability']
});
```

**How agents use this**:
1. Study the extraction pattern shown in the example
2. Call the extractData function with your own selectors and field mappings
3. **Example files are for reference** - call functions with different parameters, don't modify examples

## Script Reference

### Quick Reference for All Scripts

| Script | Language | Usage Type | Purpose | How Agent Uses It |
|--------|----------|------------|---------|-------------------|
| api-client.ts | TypeScript | TEMPLATE | API client template | Copy to project with helpers, customize |
| http-helpers.ts | TypeScript | TEMPLATE | Helper functions | Copy to project with main template |
| scraper.js | JavaScript | EXECUTABLE | Web scraping tool | Execute from .agents/skills/, consume output |
| jwt-example.ts | TypeScript | EDUCATIONAL | JWT auth pattern | Study example, install jsonwebtoken, implement fresh |
| examples/login-flow.js | JavaScript | EDUCATIONAL | Login pattern demo | Study pattern, implement in project code |

## References
- Official documentation links
- Tutorials used
- Stack Overflow discussions
- Blog posts or guides
```

### learnings.md Format

The `learnings.md` file captures practical insights from actual usage:

```markdown
# Learnings - [Skill Name]

## Quick Tips
- Tip 1: Concise actionable tip (1 line)
- Tip 2: Another quick insight
- Tip 3: Performance optimization discovered

## Common Issues Encountered
- Issue: Brief description
  - Solution: What worked
  - When: Context where this occurred

- Issue: Another problem hit
  - Solution: How it was solved
  - Why: Root cause explanation (1 line)

## Adaptations That Worked
- Adaptation: Description of modification
  - Context: When/why needed
  - Result: Outcome achieved

## Anti-Patterns Discovered
- Anti-pattern: What NOT to do
  - Why: Why it fails
  - Instead: What to do instead

## Production Gotchas
- Gotcha: Unexpected behavior in production
  - Impact: What broke
  - Fix: How to avoid/handle

## Performance Insights
- Insight: What was learned about performance
  - Measurement: Actual metrics/impact
  - Recommendation: Action to take

## Edge Cases
- Edge case: Unusual scenario encountered
  - Handling: How to deal with it
  - Test: How to test for it

## Integration Notes
- Integration with [system/tool]
  - Issue: What to watch for
  - Solution: How to integrate correctly
```

**Format Guidelines for learnings.md**:
- ✅ Keep entries VERY concise (1-3 lines each)
- ✅ Focus on actionable insights
- ✅ Include context (when/where it matters)
- ✅ Date entries if tracking over time
- ❌ No lengthy explanations (belongs in skill.md)
- ❌ No obvious information
- ❌ No duplicating what's in skill.md

**Example learnings.md**:
```markdown
# Learnings - Playwright Web Interaction

## Quick Tips
- Add `await page.waitForLoadState('networkidle')` after navigation for dynamic sites
- Use `page.locator()` instead of `page.$()` for better error messages
- Set timeout to 60000ms for slow-loading pages: `{ timeout: 60000 }`

## Common Issues Encountered
- Issue: Screenshots fail on headless mode in Docker
  - Solution: Add `--disable-dev-shm-usage` flag to browser launch
  - When: Docker containers with limited /dev/shm

- Issue: Selectors break after site update
  - Solution: Use data-testid attributes instead of CSS classes
  - Why: CSS classes change frequently, test IDs are stable

## Adaptations That Worked
- Adaptation: Added retry logic for flaky network calls
  - Context: E-commerce site with unreliable API
  - Result: Success rate improved from 80% to 99%

## Anti-Patterns Discovered
- Anti-pattern: Using `page.waitForTimeout(5000)` for waiting
  - Why: Unreliable and slow tests
  - Instead: Use `page.waitForSelector()` or `page.waitForLoadState()`

## Performance Insights
- Insight: Reusing browser context saves 2-3 seconds per run
  - Measurement: Test suite went from 45s to 32s
  - Recommendation: Launch browser once, reuse context for multiple pages
```

## Workflow: Skill Identification and Creation

### Phase 1: Skill Need Identification (Sub-Agent)

When a sub-agent reviews a specification and encounters a knowledge gap:

1. ✅ **Think deeply**: Can this be solved with existing knowledge?
2. ✅ **Check existing skills**: Scan `.agents/skills/` directory frontmatter
3. ✅ **Search for information**: Use search tool if available to understand the concept
4. ✅ **Consult internet**: Research best practices, official docs, tutorials
5. ✅ **Evaluate alternatives**: Are there other ways to achieve the task?

**Decision Tree**:
```
Do I understand how to accomplish this task?
├─ YES → Proceed with implementation
└─ NO → Can I find existing skill that covers this?
          ├─ YES → Use existing skill (if approved)
          └─ NO → Can I learn from quick research?
                    ├─ YES → Research, understand, proceed
                    └─ NO → Must create new skill
```

### Phase 2: Skill Creation (Sub-Agent)

**ONLY** if skill creation is necessary:

1. ✅ **Research thoroughly**:
   - Use search tool to find official documentation
   - Consult multiple sources for best practices
   - Understand the concept deeply before documenting
   - Find code examples and working implementations

2. ✅ **Create skill directory**:
   ```bash
   mkdir -p .agents/skills/[NN-skill-name]
   mkdir -p .agents/skills/[NN-skill-name]/examples  # if needed
   ```

3. ✅ **Create supporting scripts and code** (if applicable):
   - Write executable scripts with clear argument interfaces (`.sh`, `.py`, `.js`, `.ts`, `.rs`)
   - Design scripts to accept parameters for different use cases
   - Create reusable code modules with exported functions
   - Build working examples demonstrating usage patterns
   - Add configuration files if needed
   - Test scripts thoroughly with various argument combinations
   - Keep code focused, well-commented, and generic

4. ✅ **Write skill.md**:
   - Start with complete frontmatter (approved: No)
   - **Clearly state Usage Type** (TEMPLATE/EXECUTABLE/EDUCATIONAL) at the top
   - Include `files` field listing all attached scripts
   - Document overview, prerequisites, concepts
   - Add "Attached Scripts and Code" section explaining each file
   - **Make instructions clear and unambiguous** (will be verified at 2 checkpoints)
   - Provide step-by-step guide referencing scripts
   - Include common patterns showing script usage
   - Add pitfalls specific to the scripts
   - Provide examples using the attached scripts
   - Add "Script Reference" table for quick lookup
   - Include references to sources
   - **CRITICAL**: Ensure clarity for Main Agent verification (Checkpoint 1) and Sub-Agent usage (Checkpoint 2)

5. ✅ **Document how agents use scripts**:
   - Clearly define all script arguments and parameters
   - Show execution examples with different argument combinations
   - Document function signatures for importable modules
   - Explain return values and output formats
   - Provide examples of calling functions with various parameters
   - State when modification requires approval

6. ✅ **Keep it focused**:
   - One skill per directory
   - Scripts should be modular and reusable
   - Stay on topic
   - Be comprehensive but not verbose
   - Use clear, technical language

7. ✅ **Report to Main Agent**:
   ```
   "Created new skill: [skill-name]
   Location: .agents/skills/[NN-skill-name]/skill.md
   Reason: [Why this skill was necessary]
   Research sources: [Links to documentation/resources used]

   Attached files:
   - browser-automation.js: Executable script for browser automation
   - scraper-module.ts: TypeScript module with scraping functions
   - examples/login-flow.js: Login flow pattern example
   - examples/data-extraction.js: Data extraction pattern example

   All scripts have been tested and are ready for use.
   Ready for review and user approval."
   ```

### Phase 3: Skill Review (Main Agent)

When Main Agent receives skill creation report:

1. ✅ **Read the skill document**:
   - Verify frontmatter is complete (including `files` field if applicable)
   - Check if content is comprehensive
   - Ensure examples are clear and correct
   - Verify "Attached Scripts and Code" section exists if files present

2. ✅ **Review attached scripts and code** (if applicable):
   - Read each script file
   - Verify scripts are well-commented
   - Check if scripts are functional and safe
   - Ensure scripts follow best practices
   - Verify no hardcoded credentials or secrets
   - Check if script usage is documented in skill.md

3. ✅ **Validate skill makes sense**:
   - Use search tool to verify information accuracy
   - Consult internet for best practices
   - Check if approach is reasonable and industry-standard
   - Verify scripts align with documented approach

4. ✅ **Assess necessity**:
   - Is this skill truly needed?
   - Could the task be done without it?
   - Is the skill well-scoped (not too broad/narrow)?
   - Are the scripts necessary and useful?

5. ✅ **Report to user**:
   ```
   "New skill documented: [skill-name]

   Location: .agents/skills/[NN-skill-name]/skill.md
   Purpose: [What the skill achieves]
   Reason needed: [Why sub-agent needed this skill]

   Attached scripts:
   - browser-automation.js (JavaScript): Executable automation script (accepts arguments)
   - scraper-module.ts (TypeScript): Reusable module with scraping functions
   - examples/login-flow.js: Working login pattern example
   - examples/data-extraction.js: Working extraction pattern example

   The skill and scripts have been reviewed and appear technically sound.
   All scripts have been tested and follow best practices.

   Please review and approve to proceed with implementation.

   To approve, I will update the frontmatter:
   approved: No → approved: Yes

   Please confirm if you'd like me to approve this skill."
   ```

### Phase 4: User Approval

**User reviews the skill and provides decision**:

**If Approved**:
- Main Agent updates frontmatter: `approved: Yes`
- Main Agent resumes or spawns sub-agent
- Sub-agent proceeds with implementation using approved skill
- Skill is now available for future use

**If Rejected or Alternative Provided**:
- User provides feedback or alternative approach
- Main Agent communicates alternative to sub-agent
- Sub-agent implements using user's guidance
- Skill file may be updated or removed

**If Needs Revision**:
- User provides specific feedback
- Main Agent communicates feedback to sub-agent
- Sub-agent updates skill document
- Process returns to Phase 3 (review)

## Skill Clarity Verification (MANDATORY)

Skills must be clear and understandable at two critical checkpoints:

### Checkpoint 1: During Requirements Creation (Main Agent)

**When**: After requirements.md is created and skills are listed in frontmatter

**Main Agent MUST**:

1. ✅ **Review each listed skill thoroughly**:
   - Read complete skill.md content
   - Verify usage type is clearly stated (TEMPLATE/EXECUTABLE/EDUCATIONAL)
   - Confirm instructions are clear and unambiguous
   - Check that all files are documented with their purpose
   - Validate that examples are understandable

2. ✅ **Verify clarity for each usage type**:

   **For TEMPLATE skills**:
   - ✅ Are ALL files that need copying clearly listed?
   - ✅ Are customization instructions clear?
   - ✅ Is it clear which files are templates vs helpers?
   - ✅ Are there clear "DO NOT import from .agents/skills/" warnings?

   **For EXECUTABLE skills**:
   - ✅ Are command-line arguments clearly documented?
   - ✅ Are return values/output formats specified?
   - ✅ Are usage examples provided?
   - ✅ Is error handling documented?

   **For EDUCATIONAL skills**:
   - ✅ Are external dependencies clearly listed?
   - ✅ Are installation commands provided?
   - ✅ Are patterns clearly explained?
   - ✅ Is it clear this is for learning, not importing?

3. ✅ **Document verification in requirements.md**:
   ```markdown
   ## Skills Clarity Verification

   **Verified by Main Agent**: [Date]

   All listed skills have been reviewed for clarity:
   - [skill-name]: TEMPLATE - Clear (all files listed, customization steps documented)
   - [skill-name]: EXECUTABLE - Clear (arguments documented, output format specified)
   - [skill-name]: EDUCATIONAL - Clear (dependencies listed, pattern well-explained)
   ```

4. ✅ **If ANY skill is unclear**:
   ```
   Main Agent to User:
   "During requirements review, I found unclear skills:

   Skill: [skill-name]
   Issue: [Specific clarity problem]
   - Missing: [What's not clear]
   - Ambiguous: [What needs clarification]
   - Incomplete: [What needs to be added]

   Options:
   1. I can update the skill with proposed improvements
   2. You can update the skill directly
   3. We can remove this skill and use alternative approach

   Requirements cannot proceed until all skills are clear."
   ```

**Main Agent MUST NOT**:
- ❌ Approve requirements with unclear skills
- ❌ Let sub-agents proceed with ambiguous skills
- ❌ Skip skill clarity verification
- ❌ Assume skills are clear without reading them

### Checkpoint 2: Before Skill Usage (Sub-Agent)

**When**: Sub-agent is about to use a skill for implementation

**Sub-Agent MUST**:

1. ✅ **Perform initial clarity check**:
   - Read skill.md completely
   - Read learnings.md if it exists
   - Verify understanding of usage type
   - Confirm all instructions are clear

2. ✅ **Validate understanding**:
   ```
   Self-check questions:
   - Do I understand what type of skill this is? (TEMPLATE/EXECUTABLE/EDUCATIONAL)
   - Do I know exactly which files to copy/execute/study?
   - Are the steps clear and unambiguous?
   - Do I understand what the expected outcome is?
   - Are there any confusing or contradictory instructions?
   ```

3. ✅ **If skill is CLEAR**: Proceed with usage

4. ✅ **If skill is UNCLEAR**: Report to Main Agent immediately
   ```
   Sub-agent to Main Agent:
   "Cannot proceed with skill: [skill-name]

   Clarity Issue: [Specific problem]
   - What's unclear: [Detailed explanation]
   - Why it's blocking: [Impact on implementation]
   - What's needed: [What would make it clear]

   Possible reasons:
   - Skill may have been updated since requirements creation
   - Instructions are ambiguous or contradictory
   - Files mentioned don't match what's actually present
   - Usage type unclear or inconsistent
   - Missing critical information

   Request: Please review and clarify before I proceed."
   ```

**Main Agent Response** (when Sub-Agent reports unclear skill):

```
Main Agent to User:
"Sub-agent reports unclear skill during implementation:

Skill: [skill-name]
Last verified: [Date from requirements]
Issue: [Sub-agent's clarity concern]

This skill may have become unclear due to:
- Updates/changes since requirements creation
- Ambiguous wording discovered during actual use
- Missing information not caught in initial review
- Inconsistencies between skill.md and actual files

Options:
1. I can propose updates to clarify the skill
2. You can update the skill directly with clarifications
3. We can use an alternative approach instead

Implementation is blocked until skill clarity is restored."
```

**Why Two Checkpoints**:
- **Checkpoint 1** (Requirements): Catch obvious clarity issues early
- **Checkpoint 2** (Implementation): Catch issues that only appear during actual usage
- Skills may change between requirements creation and implementation
- Different perspectives (planning vs. doing) reveal different clarity issues

**Benefits**:
- ✅ No ambiguous skills reach implementation phase
- ✅ Skills stay maintainable and clear over time
- ✅ User has visibility when skills need improvement
- ✅ Prevents wasted implementation effort with unclear guidance
- ✅ Creates feedback loop for skill quality improvement

### Phase 5: Skill Usage (Sub-Agent)

Once skill is approved AND verified as clear:

1. ✅ **Reference skill in specification**:
   - Update specification frontmatter with skill list
   - Add to `skills` field in requirements.md frontmatter

2. ✅ **Apply skill during implementation**:
   - Check skill usage type first (TEMPLATE/EXECUTABLE/EDUCATIONAL)
   - Follow documented patterns and examples
   - **Use skill based on its type**:
     * TEMPLATE: Copy ALL files to project and customize
     * EXECUTABLE: Run scripts as external tools
     * EDUCATIONAL: Install external libraries, implement fresh code
   - Document new insights in learnings.md after completion

**How to use different skill types**:

**Type 1: TEMPLATE Skills** (Copy all files and customize)
```bash
# Step 1: Copy ALL skill files to project (template + helpers + configs)
cp .agents/skills/05-rest-api/api-client.ts ./src/clients/product-api-client.ts
cp .agents/skills/05-rest-api/http-helpers.ts ./src/clients/http-helpers.ts
cp .agents/skills/05-rest-api/config.yaml ./src/clients/api-config.yaml

# Step 2: Customize the COPIED files in your project
# - Modify for your specific use case
# - Add domain-specific logic
# - Update configurations

# Step 3: Use in project (import from project location, NOT .agents/skills/)
# In src/main.ts:
import { ProductApiClient } from './clients/product-api-client';
import { handleHttpError } from './clients/http-helpers';

const client = new ProductApiClient();
const data = await client.getProducts();
```

**CRITICAL for TEMPLATE skills**:
- ✅ Copy ALL files (templates, helpers, everything)
- ✅ Customize copied files in project
- ✅ Import from project locations
- ❌ **NEVER import from `.agents/skills/` in project code**
- ❌ Original skill files remain in `.agents/skills/` untouched

---

**Type 2: EXECUTABLE Skills** (Run as external tools)
```bash
# Execute skill script from .agents/skills/ location
node .agents/skills/01-scraper/scraper.js \
  --url "https://example.com/products" \
  --selector ".product-item" \
  --output ./data/products.json

# Different task, different arguments
python .agents/skills/02-validator/validate.py \
  --schema ./schemas/user.json \
  --data ./data/users.json

# Then use the output in your project code
# In src/data-processor.ts:
import fs from 'fs';
const products = JSON.parse(fs.readFileSync('./data/products.json', 'utf8'));
```

**CRITICAL for EXECUTABLE skills**:
- ✅ Execute scripts from `.agents/skills/` location
- ✅ Scripts stay external to project
- ✅ Consume script output/results in project code
- ❌ Never copy or modify executable scripts
- ❌ Scripts are tools, not part of project codebase

---

**Type 3: EDUCATIONAL Skills** (Learn and implement fresh)
```bash
# Step 1: Install external dependencies specified in skill
npm install jsonwebtoken bcrypt express-validator
npm install @types/jsonwebtoken @types/bcrypt --save-dev

# Step 2: Study the skill examples
# Read: .agents/skills/03-auth/jwt-example.ts
# Read: .agents/skills/03-auth/password-hashing-example.ts

# Step 3: Implement FRESH code in your project following the patterns
# In src/auth/jwt-service.ts:
import jwt from 'jsonwebtoken';  // From NPM, NOT from .agents/skills/
import bcrypt from 'bcrypt';      // From NPM, NOT from .agents/skills/

export class AuthService {
  // Implement pattern learned from skill
  async hashPassword(password: string): Promise<string> {
    return bcrypt.hash(password, 10);
  }

  generateToken(userId: string): string {
    return jwt.sign({ userId }, this.secret, { expiresIn: '24h' });
  }

  // ... implement other patterns from skill
}
```

**CRITICAL for EDUCATIONAL skills**:
- ✅ Install external libraries (NPM, PyPI, Cargo, etc.)
- ✅ Study skill examples to learn patterns
- ✅ Write fresh implementation in project
- ✅ Import from external libraries, NOT from `.agents/skills/`
- ❌ **NEVER import from `.agents/skills/` directory**
- ❌ Skill examples are for learning, not importing

---

**If skill file modification is needed**:
```markdown
1. Agent identifies limitation in skill
2. Agent proposes changes to skill files
3. Report to Main Agent with proposed modifications
4. Main Agent reviews and tests changes
5. User approves modification
6. Skill files are updated in `.agents/skills/`
7. Agent uses updated skill
```

## Skill Updates and Maintenance

### Who Can Update Skills

**skill.md (Canonical Documentation)**:
- ✅ **Main Agent** can update (requires user approval)
- ✅ **Sub-Agent** can update (requires Main Agent review + user approval)
- ✅ **User** can update directly (no approval needed)
- ❌ Never update without approval process

**learnings.md (Practical Insights)**:
- ✅ **Main Agent** can update (requires user approval)
- ✅ **Sub-Agent** can update (requires Main Agent review + user approval)
- ✅ **User** can update directly (no approval needed)
- ❌ Never update without approval process

**Scripts and code**:
- ✅ **Main Agent** can update (requires user approval)
- ✅ **Sub-Agent** can update (requires Main Agent review + user approval)
- ✅ **User** can update directly (no approval needed)
- ❌ Never update without approval process

### When to Update skill.md

Update canonical documentation when:
- ✅ Fundamental approach changes
- ✅ Better pattern discovered
- ✅ Official documentation updated
- ✅ Scripts added or modified significantly
- ✅ Prerequisites change
- ✅ Core concepts need clarification

**DO NOT update skill.md for**:
- ❌ One-off issues (use learnings.md)
- ❌ Project-specific adaptations (use learnings.md)
- ❌ Temporary workarounds (use learnings.md)
- ❌ Performance tips (use learnings.md)

### When to Update learnings.md

Update practical insights when:
- ✅ Discovered new edge case
- ✅ Found better workaround
- ✅ Encountered production issue
- ✅ Learned performance optimization
- ✅ Discovered integration gotcha
- ✅ Found anti-pattern to avoid

**Learnings update is lightweight**:
- Append new insight to appropriate section
- Keep format concise (1-3 lines)
- No approval needed for minor additions

### Skill Update Workflow

**For skill.md or script changes**:

1. **Agent identifies need for update**:
   - While using skill, discovers fundamental issue
   - Better approach found in research
   - Script needs modification

2. **Agent creates/modifies files**:
   - Update skill.md content
   - Modify scripts if needed
   - Test all changes

3. **Agent reports to Main Agent** (if Sub-Agent):
   ```
   "Skill update needed: [skill-name]
   Location: .agents/skills/[NN-skill-name]/

   Changes made:
   - skill.md: Updated Step 2 with corrected approach
   - browser-automation.js: Fixed timeout handling

   Reason: Current approach fails with slow networks

   Testing: All scripts tested and working
   Ready for review and user approval."
   ```

4. **Main Agent reviews changes**:
   - Verify changes are correct
   - Test updated scripts
   - Validate against documentation
   - Ensure no breaking changes

5. **Main Agent reports to user**:
   ```
   "Skill update proposed: [skill-name]

   Location: .agents/skills/[NN-skill-name]/

   Changes:
   - skill.md: Corrected timeout handling approach
   - browser-automation.js: Added retry logic for slow networks

   Reason: Current version fails on slow network connections

   Impact: Improves reliability, no breaking changes

   Changes have been reviewed and tested.
   Please approve to update the skill."
   ```

6. **User approves or requests changes**

7. **Main Agent confirms update** (if approved)

**For learnings.md updates**:

1. **Agent discovers practical insight during usage**

2. **Agent updates learnings.md**:
   - Appends to appropriate section
   - Keeps format concise
   - Includes context

3. **Agent reports to Main Agent** (if Sub-Agent):
   ```
   "Updated learnings for skill: [skill-name]
   Added insight: [Brief description]
   Reason: [Why this is valuable]"
   ```

4. **Main Agent reviews and reports to user**:
   ```
   "Skill learning added: [skill-name]

   New insight: 'Use data-testid for selectors - CSS classes break on updates'

   This practical tip will help avoid selector failures.
   Please approve this learning addition."
   ```

5. **User approves**

6. **Learning is retained in learnings.md**

### Update Approval Requirements

**MANDATORY approval needed for**:
- ✅ Any change to skill.md content
- ✅ Any change to scripts or code
- ✅ Any change to learnings.md
- ✅ Adding new files to skill directory
- ✅ Removing files from skill directory

**Approval process**:
1. Agent makes changes
2. Main Agent reviews (validates correctness)
3. User reviews and approves
4. Changes are finalized

**Why approval is mandatory**:
- Skills are shared knowledge resources
- Incorrect skills can cause widespread issues
- Scripts execute code (security concern)
- Learnings affect future implementations
- User maintains quality control

### Validation Before Approval

**Main Agent MUST validate**:

For skill.md updates:
- ✅ Information is accurate (verify with search/internet)
- ✅ Changes improve the skill
- ✅ No introduction of errors
- ✅ Format and structure maintained
- ✅ Examples are correct

For script updates:
- ✅ Code is functional and tested
- ✅ No hardcoded secrets or credentials
- ✅ Follows best practices
- ✅ Well-commented
- ✅ No breaking changes (or documented if needed)

For learnings.md updates:
- ✅ Insight is valuable and actionable
- ✅ Format is concise (1-3 lines)
- ✅ Context is provided
- ✅ Not duplicating skill.md content
- ✅ Actually learned from usage (not speculation)

## Specification Integration

### Skills Field in Specifications

Specifications **MUST** list required skills in frontmatter:

```markdown
---
status: in-progress
priority: high
completed: 2
uncompleted: 8
skills:
  - playwright-web-interaction
  - jwt-authentication
tools:
  - TypeScript
  - Playwright
  - Node.js
---
```

### When to Update

**Add skills to specification when**:
- Sub-agent identifies skill need during planning
- Skill is approved and will be used
- Update occurs BEFORE implementation begins

**Direction of reference**:
- ✅ Specifications reference skills (one-way relationship)
- ✅ Skills are independent documents
- ❌ Skills do NOT track which specs use them
- ❌ No `related_specs` field in skill frontmatter

## Skill Scanning (Agents)

### Efficient Skill Discovery

To save context space, agents **MUST**:

1. ✅ **Scan only frontmatter** initially:
   ```bash
   # Read just the frontmatter of all skills
   for skill in .agents/skills/*/skill.md; do
     head -n 15 "$skill"  # Read just frontmatter
   done
   ```

2. ✅ **Match by name and description**:
   - Use `name` and `description` to identify relevant skills
   - Don't read full content unless skill is selected for use

3. ✅ **Check approval status**:
   - Only consider skills with `approved: Yes`
   - Report to Main Agent if needed skill is unapproved

4. ✅ **Read full content only when using**:
   - Once skill is selected, read complete document
   - Apply detailed guidance during implementation

### Skill Discovery Example

```
Sub-agent task: Implement web scraping for product data

1. Scan .agents/skills/ frontmatter
2. Find: "01-playwright-web-interaction" - approved: Yes
3. Description matches: "automate browser interactions, scraping"
4. Read full skill.md content
5. Apply skill during implementation
6. Update specification with skill reference
```

## Enforcement and Violations

### Sub-Agent Requirements

Sub-agents **MUST**:
- ✅ Think deeply before creating skills
- ✅ Exhaust research and alternatives first
- ✅ Create comprehensive, accurate skill documents
- ✅ Test all scripts before reporting
- ✅ Clearly mark skill with **Usage Type** (TEMPLATE/EXECUTABLE/EDUCATIONAL)
- ✅ Check Usage Type before using any skill
- ✅ **Perform clarity check before using any skill** (Checkpoint 2)
- ✅ **Report unclear skills to Main Agent immediately**
- ✅ Copy ALL files (templates + helpers) for TEMPLATE skills
- ✅ Customize copied files in project directory
- ✅ Execute EXECUTABLE scripts as external tools
- ✅ Study EDUCATIONAL skills and implement fresh code
- ✅ Install external dependencies (NPM, PyPI, etc.) for EDUCATIONAL skills
- ✅ Import from PROJECT locations, NEVER from `.agents/skills/`
- ✅ Report skill creation to Main Agent
- ✅ Never use unapproved skills
- ✅ Update specifications with skill references
- ✅ Scan skills directory before starting work
- ✅ Read learnings.md when using a skill (not during scan)
- ✅ Update learnings.md after using skill (if insights gained)
- ✅ Report skill updates to Main Agent for approval

Sub-agents **MUST NOT**:
- ❌ Create skills for trivial or known tasks
- ❌ Skip research when creating skills
- ❌ Create vague or incomplete skill documents
- ❌ Use skills before user approval
- ❌ Proceed with unapproved skills without reporting
- ❌ Create duplicate skills without checking existing ones
- ❌ Read learnings.md during frontmatter scan (waste of context)
- ❌ Update skill.md or scripts without approval
- ❌ Add untested scripts to skills
- ❌ **Proceed with unclear skills without reporting** (CRITICAL)
- ❌ **Skip clarity check before using skills** (CRITICAL)
- ❌ **Import from `.agents/skills/` in project code** (CRITICAL VIOLATION)
- ❌ Copy only some files from TEMPLATE skills (must copy ALL)
- ❌ Modify EXECUTABLE scripts (run as external tools only)
- ❌ Import skill example code for EDUCATIONAL skills (implement fresh)
- ❌ Modify original files in `.agents/skills/` directory
- ❌ Guess at usage type - always check skill.md documentation

### Main Agent Requirements

Main Agent **MUST**:
- ✅ Review all skill documents thoroughly
- ✅ Review all attached scripts for safety and correctness
- ✅ Verify skill has clear **Usage Type** (TEMPLATE/EXECUTABLE/EDUCATIONAL)
- ✅ **Verify ALL skills for clarity during requirements creation** (Checkpoint 1)
- ✅ **Document skill clarity verification in requirements.md**
- ✅ **Block requirements if any skill is unclear**
- ✅ **Report unclear skills to user with specific issues**
- ✅ **Relay sub-agent clarity concerns to user** (Checkpoint 2)
- ✅ Validate that TEMPLATE skills list ALL files to copy
- ✅ Validate that EDUCATIONAL skills specify external dependencies
- ✅ Ensure no imports from `.agents/skills/` in example code
- ✅ Validate skill accuracy using search/internet
- ✅ Report new skills to user for approval
- ✅ Report skill updates to user for approval
- ✅ Update frontmatter when user approves
- ✅ Block implementation until approval received
- ✅ Communicate alternatives if user provides them
- ✅ Validate learnings.md updates before user approval
- ✅ Ensure sub-agents respect Usage Type boundaries
- ✅ Verify sub-agents don't import from `.agents/skills/` in project

Main Agent **MUST NOT**:
- ❌ Approve skills without user consent
- ❌ Skip validation of skill content
- ❌ Skip validation of scripts and code
- ❌ Allow sub-agents to use unapproved skills
- ❌ Proceed without user approval
- ❌ Allow skill updates without user approval
- ❌ Approve skills with ambiguous Usage Type documentation
- ❌ Allow sub-agents to import from `.agents/skills/` in project code
- ❌ Allow partial copying of TEMPLATE skills (must copy ALL files)
- ❌ **Skip skill clarity verification during requirements** (CRITICAL)
- ❌ **Approve requirements with unclear skills** (CRITICAL)
- ❌ **Ignore sub-agent clarity concerns** (CRITICAL)

### Critical Violations

**Serious violations**:
- Using unapproved skills (agent continues without approval)
- Creating skills for trivial tasks (wastes time and context)
- Skipping research (poor quality skills)
- Not reporting to Main Agent (bypasses review)
- Not updating specifications (lost traceability)
- Updating skill.md or scripts without approval (quality control bypass)
- Reading learnings.md during frontmatter scan (context waste)
- Adding untested scripts (safety risk)
- **Skipping skill clarity verification during requirements** (CRITICAL - Main Agent)
- **Approving requirements with unclear skills** (CRITICAL - Main Agent)
- **Proceeding with unclear skills without reporting** (CRITICAL - Sub-Agent)
- **Ignoring sub-agent clarity concerns** (CRITICAL - Main Agent)
- **Importing from `.agents/skills/` in project code** (CRITICAL - breaks isolation)
- **Partial copying of TEMPLATE skills** (must copy ALL files including helpers)
- **Modifying EXECUTABLE scripts** instead of running as tools (breaks reusability)
- **Importing skill example code** for EDUCATIONAL skills (should implement fresh)
- **Modifying original files in `.agents/skills/` directory** (corrupts shared resources)
- **Using skills without checking Usage Type** (incorrect usage pattern)

**If agent needs unapproved skill**:
```
Sub-agent: "Cannot proceed. Required skill not approved:
  Skill: .agents/skills/03-kubernetes-deployment/skill.md
  Status: approved: No
  Reason needed: [Explanation]

Awaiting user approval to continue."

Main Agent: Reports to user, requests approval
User: Provides approval or alternative
Main Agent: Updates skill or provides guidance
Sub-agent: Resumes with approved approach
```

## Examples

### Example 1: Creating Playwright Skill

**Scenario**: Agent needs to scrape data from dynamic website

```markdown
# Sub-agent thinking process:
1. Task: Extract product data from JavaScript-rendered pages
2. Knowledge gap: Haven't used Playwright for web scraping
3. Check .agents/skills/: No existing Playwright skill
4. Research: Read Playwright docs, tutorials, examples
5. Decision: Need comprehensive Playwright skill
6. Create: .agents/skills/01-playwright-web-interaction/skill.md

# Skill document (condensed):
---
name: "Playwright Web Interaction"
description: "Guide for using Playwright to automate browser interactions, scraping, and testing"
approved: No
created: 2026-01-13
tools:
  - Playwright
  - TypeScript
---

# Playwright Web Interaction

## Overview
Playwright is a browser automation library for testing and scraping...

## When to Use
- Scraping JavaScript-rendered content
- Testing web applications
- Automating browser interactions

## Step-by-Step Guide
### Step 1: Installation
...

### Step 2: Basic Navigation
...

[Continue with comprehensive guide]

# Report to Main Agent:
"Created new skill: playwright-web-interaction
Location: .agents/skills/01-playwright-web-interaction/skill.md
Reason: Need to scrape dynamic website with JavaScript rendering
Research: Playwright official docs, several tutorials
Ready for review and approval."
```

### Example 2: Using Existing Approved Skill

**Scenario**: Agent needs JWT authentication

```markdown
# Sub-agent thinking process:
1. Task: Implement user authentication
2. Check .agents/skills/: Found "03-jwt-authentication"
3. Read frontmatter:
   - name: "JWT Authentication Implementation"
   - description: "Guide for implementing JWT-based auth..."
   - approved: Yes  ← APPROVED!
4. Read full skill.md content
5. Apply skill patterns during implementation
6. Update specification frontmatter:
   skills:
     - jwt-authentication

# No user approval needed - skill already approved
# Proceed with implementation
```

### Example 3: Using TEMPLATE Skill (Copy All Files and Customize)

**Scenario**: Agent needs to create custom API client from template

```markdown
# Sub-agent task: Create REST API client for third-party service

1. Check .agents/skills/: Found "05-rest-api-client-pattern"
2. Read frontmatter: approved: Yes
3. Read full skill.md content
4. skill.md states: "TEMPLATE skill - Copy all files to your project and customize"

# From skill.md:
---
name: "REST API Client Pattern"
description: "Template-based pattern for creating type-safe REST API clients"
approved: Yes
usage_type: TEMPLATE
files:
  - api-client.ts: "TEMPLATE: Main API client implementation"
  - http-helpers.ts: "HELPER: HTTP utility functions (copy with template)"
  - retry-logic.ts: "HELPER: Retry mechanisms (copy with template)"
  - config.example.yaml: "CONFIG: Example configuration file"
---

## Usage Instructions

**Skill Type**: TEMPLATE - Copy ALL files to project and customize

This is a template skill. Copy ALL files to your project:
1. api-client.ts - Main template
2. http-helpers.ts - Helper functions
3. retry-logic.ts - Retry utilities
4. config.example.yaml - Configuration template

Customize all copied files for your specific API.

**CRITICAL**: Do NOT import from `.agents/skills/` in your project code. Copy files to project first, then import from project locations.

# Agent execution:
# Step 1: Copy ALL files to project
cp .agents/skills/05-rest-api-client-pattern/api-client.ts ./src/clients/product-api-client.ts
cp .agents/skills/05-rest-api-client-pattern/http-helpers.ts ./src/clients/http-helpers.ts
cp .agents/skills/05-rest-api-client-pattern/retry-logic.ts ./src/clients/retry-logic.ts
cp .agents/skills/05-rest-api-client-pattern/config.example.yaml ./config/api-config.yaml

# Step 2: Modify ALL COPIED files in project (NOT originals in .agents/skills/)
# In src/clients/product-api-client.ts:
# - Change baseURL to product API
# - Add product-specific endpoints
# - Update types for product schema

# In src/clients/http-helpers.ts:
# - Customize error handling for product API
# - Add product-specific error codes

# In src/clients/retry-logic.ts:
# - Adjust retry delays for product API
# - Customize backoff strategy

# Step 3: Use in project (import from PROJECT, not .agents/skills/)
# In src/main.ts:
import { ProductApiClient } from './clients/product-api-client';
import { handleHttpError, retryRequest } from './clients/http-helpers';
import { withRetry } from './clients/retry-logic';

const client = new ProductApiClient();
const data = await client.getProduct('123');

// All imports are from project locations - NEVER from .agents/skills/

# Report to Main Agent:
"Implementation complete using skill: rest-api-client-pattern

Used approach:
- Copied ALL skill files to project (4 files total)
  * api-client.ts → src/clients/product-api-client.ts
  * http-helpers.ts → src/clients/http-helpers.ts
  * retry-logic.ts → src/clients/retry-logic.ts
  * config.example.yaml → config/api-config.yaml
- Customized all files for Product API (endpoints, types, error handling)
- All imports are from project locations
- Original skill files in .agents/skills/ remain untouched
- NO imports from .agents/skills/ in project code

Result: Type-safe Product API client with custom retry logic"
```

**Key Points**:
- ✅ **TEMPLATE skill** - Copy ALL files (template + ALL helpers)
- ✅ **Customize** all copied files for specific use case
- ✅ **Import from project** locations in your code
- ❌ **NEVER import from `.agents/skills/`** in project code
- ❌ **Original files** in `.agents/skills/` remain completely untouched
- ✅ `.agents/skills/` is isolated knowledge base, not part of project

### Example 4: Unapproved Skill Blocking Work

**Scenario**: Agent encounters unapproved skill

```markdown
# Sub-agent discovers:
1. Task: Deploy to Kubernetes cluster
2. Found skill: .agents/skills/04-kubernetes-deployment/skill.md
3. Check frontmatter: approved: No
4. Cannot proceed without approval

# Report to Main Agent:
"Cannot continue implementation.
Required skill: kubernetes-deployment
Status: NOT APPROVED (approved: No)
Location: .agents/skills/04-kubernetes-deployment/skill.md

This skill is critical for deploying the application.
Awaiting user approval to proceed."

# Main Agent to user:
"Sub-agent requires unapproved skill: kubernetes-deployment

The agent needs this to deploy the application to Kubernetes.
Skill location: .agents/skills/04-kubernetes-deployment/skill.md

Please review the skill document and let me know if:
1. You approve this skill (I'll update approved: Yes)
2. You have an alternative approach
3. You'd like revisions to the skill

Work is blocked until this is resolved."
```

## Integration with Other Rules

### Rule 04 (Agent Orchestration)
- Skills are identified during "Before Starting Work" phase
- Skills are created after reading specifications
- Skills are applied during "During Work" phase
- Main Agent coordinates skill approval

### Rule 06 (Specifications and Requirements)
- Specifications frontmatter includes `skills` field
- Skills are documented in requirements.md
- Related specs listed in skill frontmatter

### Rule 07 (Language Conventions)
- Skills may be stack-specific (e.g., Rust async patterns)
- Skills complement stack documentation
- Skills are more task-focused than stack docs

## Benefits

### For Agents
- ✅ Clear guidance for complex tasks
- ✅ Reusable knowledge across projects
- ✅ Structured approach to learning
- ✅ Reduced errors from misunderstanding

### For Users
- ✅ Control over methodologies used
- ✅ Visibility into agent knowledge gaps
- ✅ Opportunity to provide better approaches
- ✅ Building reusable knowledge base

### For Projects
- ✅ Documented institutional knowledge
- ✅ Consistency across implementations
- ✅ Faster onboarding for new work
- ✅ Traceable decisions and approaches

## Summary

**Core Workflow**:
```
Need identified → Research → Check existing skills →
Create if necessary → Main Agent review → User approval →
Update approved: Yes →
Checkpoint 1: Main Agent verifies clarity during requirements →
Document verification in requirements.md →
Checkpoint 2: Sub-Agent verifies clarity before usage →
Implementation proceeds →
Update learnings.md with insights
```

**Key Principles**:
- Skills are last resort (think deeply first)
- User approval is mandatory (creation AND updates)
- Research thoroughly before creating
- Scan efficiently (frontmatter only, skip learnings.md)
- Specifications reference skills (one-way relationship)
- Never proceed with unapproved skills
- skill.md is canonical (formal documentation)
- learnings.md captures practical insights (read when using, not scanning)
- **Two mandatory clarity checkpoints**:
  - Checkpoint 1: Main Agent verifies during requirements creation
  - Checkpoint 2: Sub-Agent verifies before skill usage
- **Three Usage Types**: TEMPLATE (copy all), EXECUTABLE (run as tool), EDUCATIONAL (learn and implement)
- **`.agents/skills/` is ISOLATED** - never import from it in project code
- **TEMPLATE skills**: Copy ALL files (templates + helpers) to project
- **EXECUTABLE skills**: Run scripts as external tools, consume output
- **EDUCATIONAL skills**: Install external libraries, implement fresh code

**File Structure**:
```
.agents/skills/[NN-skill-name]/
├── skill.md (required - canonical, always approved before use)
├── learnings.md (optional - practical insights, read when using)
├── template-main.ts (TEMPLATE: copy to project with all helpers)
├── template-helpers.ts (TEMPLATE: copy with main template)
├── executable-tool.sh (EXECUTABLE: run as external command)
├── example-pattern.ts (EDUCATIONAL: study and implement fresh)
└── examples/ (optional - working examples for reference)
```

**Three Usage Types**:
1. **TEMPLATE**: Copy ALL files (template + helpers) to project, customize copied files
2. **EXECUTABLE**: Run scripts from `.agents/skills/` as external tools
3. **EDUCATIONAL**: Study examples, install external libraries (NPM/PyPI/etc.), implement fresh

**CRITICAL - Skills Directory Isolation**:
- ❌ **NEVER `import/require/use` from `.agents/skills/` in project code**
- ✅ `.agents/skills/` is knowledge base + tool collection, NOT a code library
- ✅ TEMPLATE: Copy to project, then import from project locations
- ✅ EXECUTABLE: Run as external tools
- ✅ EDUCATIONAL: Implement fresh using external libraries

**Critical Rules**:
- ❌ No skill usage without approval
- ❌ No casual skill creation
- ❌ No skill updates without approval
- ❌ Skills do NOT track specifications (no related_specs field)
- ❌ Do NOT read learnings.md during frontmatter scan
- ❌ **NEVER skip clarity verification** (both checkpoints mandatory)
- ❌ **NEVER proceed with unclear skills**
- ❌ **NEVER import from `.agents/skills/` in project code** (CRITICAL)
- ❌ **NEVER partially copy TEMPLATE skills** (must copy ALL files)
- ❌ **NEVER modify EXECUTABLE scripts** (run as external tools)
- ❌ **NEVER import EDUCATIONAL examples** (implement fresh)
- ❌ **NEVER modify original files in `.agents/skills/`**
- ✅ Always report to Main Agent
- ✅ Always update specifications with skill references
- ✅ Always validate before approval (including scripts)
- ✅ Always research thoroughly
- ✅ Always test scripts before submitting
- ✅ Read learnings.md when actively using skill
- ✅ Update learnings.md after gaining insights (with approval)
- ✅ **Checkpoint 1: Main Agent verifies clarity during requirements**
- ✅ **Checkpoint 2: Sub-Agent verifies clarity before usage**
- ✅ **Report unclear skills immediately to user**
- ✅ **Check Usage Type before using any skill**
- ✅ **TEMPLATE: Copy ALL files to project and customize**
- ✅ **EXECUTABLE: Run scripts as external tools**
- ✅ **EDUCATIONAL: Install external libs, implement fresh**

**When to Update**:
- **skill.md**: Fundamental changes, better patterns, official docs update (requires approval)
- **learnings.md**: Practical insights, edge cases, workarounds, tips (requires approval)
- **Scripts**: Bug fixes, improvements, new functionality (requires approval)

**Documentation Hierarchy**:
1. **skill.md** = Canonical truth (what to know BEFORE using)
2. **learnings.md** = Practical wisdom (what you learn AFTER using)
3. **Files** = Implementation support (how to DO it)
   - **TEMPLATE**: Copy ALL to project and customize
   - **EXECUTABLE**: Run as external tool
   - **EDUCATIONAL**: Learn pattern, implement with external libraries

---
*Created: 2026-01-13*
*Last Updated: 2026-01-13 (Added learnings.md, skill updates, canonical documentation concept, three usage types, skills directory isolation, and mandatory clarity verification checkpoints)*
