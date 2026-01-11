# Language Conventions and Standards

## Purpose
This rule establishes mandatory language-specific coding standards and conventions that all agents must follow when implementing logic in any programming language. It ensures consistency, quality, and adherence to best practices while creating a self-improving system where agents learn from mistakes and update standards.

## Rule

### Language Identification Requirement
Before any implementation work begins, the agent responsible for gathering specifications and creating requirements **MUST**:

1. **Identify Programming Languages**
   - Clearly identify which programming language(s) will be used
   - Document this in the `requirements.md` file
   - Common languages include: Rust, JavaScript/TypeScript, Python, Go, etc.

2. **Document Language Stack**
   - Add a "Language Stack" section to `requirements.md`
   - List all languages that will be used in the implementation
   - Specify which parts of the system will use which language
   - Example: "Backend API in Rust, Frontend UI in TypeScript, Build scripts in JavaScript"

3. **Document in Tasks File**
   - Include language information in `tasks.md` frontmatter under `tools`
   - Example: `tools: ["Rust", "TypeScript", "Jest", "Cargo"]`

### Stack Standards Directory Structure

```
.agents/
├── stacks/
│   ├── javascript.md          # JavaScript/TypeScript standards
│   ├── rust.md                # Rust standards
│   ├── python.md              # Python standards
│   ├── go.md                  # Go standards
│   └── [language].md          # Additional language standards as needed
```

### Stack Standards File Requirements

Each language stack file **MUST** contain:

1. **Language Overview**
   - Language name and version requirements
   - Use cases in this project
   - When to use this language vs others

2. **Coding Standards**
   - Code formatting rules
   - Naming conventions (variables, functions, classes, files, etc.)
   - Code organization patterns
   - Comment and documentation requirements

3. **Best Practices**
   - Idiomatic patterns for this language
   - Error handling approach
   - Testing requirements
   - Performance considerations
   - Security practices

4. **What Constitutes Valid Code**
   - Code quality requirements
   - Required checks (linting, formatting, type checking)
   - Test coverage expectations
   - Documentation requirements

5. **Common Pitfalls**
   - Known mistakes to avoid
   - Anti-patterns specific to this language
   - Common bugs and how to prevent them

6. **Tools and Configuration**
   - Required tools (linters, formatters, test runners)
   - Configuration file requirements
   - Build system requirements
   - Dependency management

7. **Examples**
   - Good code examples
   - Bad code examples (what NOT to do)
   - Common patterns and how to implement them

8. **Learning Log**
   - Record of lessons learned
   - Mistakes that were made and corrected
   - New patterns or practices discovered
   - Date-stamped entries showing evolution of standards

## Mandatory Requirements

### For Specification Agents

When creating specifications in `requirements.md`, agents **MUST**:

1. **Add Language Stack Section**
   ```markdown
   ## Language Stack

   This specification will be implemented using the following languages:

   - **Rust**: Backend API implementation
     - Version: 1.75+
     - Purpose: High-performance, type-safe backend logic
     - See: `.agents/stacks/rust.md`

   - **TypeScript**: Frontend UI components
     - Version: 5.0+
     - Purpose: Type-safe React components
     - See: `.agents/stacks/javascript.md`
   ```

2. **Reference Stack Standards**
   - Include clear references to relevant stack standard files
   - Mention that agents MUST read these files before implementation
   - Note that deviations from standards are not allowed

3. **Document Language-Specific Requirements**
   - Call out any language-specific constraints or requirements
   - Note performance requirements tied to language choice
   - Specify integration requirements between different languages

### For Implementation Agents

Before writing ANY code, agents **MUST**:

1. **Read Relevant Stack Standards**
   - Load and read the `.agents/stacks/[language].md` file for each language being used
   - Understand all coding standards and requirements
   - Internalize best practices and anti-patterns

2. **Follow Standards Strictly**
   - **ZERO TOLERANCE** for deviations from documented standards
   - Code MUST conform to all requirements in the stack file
   - If unsure about a standard, stop and ask for clarification
   - Never "improvise" or "use own judgment" against documented standards

3. **Verify Compliance**
   - Run all required linters and formatters
   - Ensure all tests pass
   - Verify documentation is complete
   - Check that code follows all naming conventions

4. **Update Stack Standards When Learning**
   - If you discover a new best practice, add it to the stack file
   - If you make a mistake, document it in the "Learning Log" section
   - If you find a better way to do something, update the examples
   - Keep standards current and accurate

### Learning Log Updates

Agents **MUST** update the Learning Log section of stack files when:

1. **Mistakes Are Made**
   - Document what the mistake was
   - Explain why it was wrong
   - Show the correct approach
   - Add date and context

2. **New Patterns Are Discovered**
   - Document the new pattern
   - Explain when to use it
   - Provide examples
   - Note performance or security benefits

3. **Standards Evolve**
   - Document changes to existing standards
   - Explain reasoning for the change
   - Update examples to reflect new standard
   - Mark outdated patterns as deprecated

4. **Tool Configuration Changes**
   - Document tool updates or configuration changes
   - Explain why the change was needed
   - Update setup instructions
   - Note compatibility requirements

### Learning Log Format

```markdown
## Learning Log

### 2026-01-11: Error Handling Pattern Update
**Issue**: Previously used `unwrap()` extensively in Rust code, causing panics in production.
**Learning**: Always use proper error handling with `Result<T, E>` and the `?` operator.
**Corrective Action**: Updated all unwrap() calls to proper error handling. Added linter rule to warn on unwrap().
**New Standard**: Never use `unwrap()` or `expect()` in production code. Always handle errors explicitly.

### 2026-01-10: TypeScript Type Safety Improvement
**Issue**: Found several `any` types in code, reducing type safety.
**Learning**: TypeScript's value comes from strong typing. Using `any` defeats this purpose.
**Corrective Action**: Replaced all `any` with proper types. Enabled strict mode in tsconfig.json.
**New Standard**: `any` type is forbidden. Use `unknown` when type is truly unknown, then narrow with type guards.
```

## Workflow Integration

### Complete Workflow with Language Standards

```
1. User Requests Feature
   ↓
2. Main Agent Conversation with User
   ├─ Identify which languages will be used
   ├─ Ask clarifying questions about language requirements
   └─ Confirm technical approach and language choices
   ↓
3. Create Specification Directory
   ├─ Create requirements.md with Language Stack section
   ├─ List all languages to be used
   ├─ Reference relevant .agents/stacks/[language].md files
   └─ Note that standards must be followed strictly
   ↓
4. Create tasks.md
   ├─ Include languages in frontmatter tools list
   └─ Create tasks for each language component
   ↓
5. Launch Review Agent
   ├─ Review agent verifies language stack is documented
   ├─ Review agent checks that stack standard files exist
   ├─ Review agent confirms requirements mention standards
   └─ Review agent reports readiness
   ↓
6. Launch Implementation Agents
   ├─ **MANDATORY**: Read .agents/stacks/[language].md FIRST
   ├─ Understand all standards and requirements
   ├─ Implement code following standards strictly
   ├─ Verify compliance with all checks
   └─ Update Learning Log if new lessons learned
   ↓
7. Code Review and Quality Check
   ├─ Verify code follows stack standards
   ├─ Run all linters and formatters
   ├─ Ensure tests pass
   └─ Check documentation completeness
   ↓
8. Update Stack Standards If Needed
   ├─ Add new learnings to Learning Log
   ├─ Update examples if better patterns found
   ├─ Document mistakes and corrections
   └─ Commit stack file updates
```

## Enforcement

### Zero Tolerance Policy

This rule has **ZERO TOLERANCE** for violations:

- ❌ **FORBIDDEN**: Writing code without reading relevant stack standard file
- ❌ **FORBIDDEN**: Deviating from documented standards without explicit approval
- ❌ **FORBIDDEN**: Ignoring coding conventions in stack files
- ❌ **FORBIDDEN**: Not updating Learning Log when mistakes are made
- ❌ **FORBIDDEN**: Creating specifications without documenting language stack
- ❌ **FORBIDDEN**: Using languages not documented in requirements

### Violation Consequences

Any agent that violates this rule will:
1. Have their code rejected immediately
2. Be required to read the stack standards file
3. Rewrite the code to comply with standards
4. Document the violation in the Learning Log
5. Report the violation to the user

### Mandatory Checks

Before ANY code is committed, the following checks **MUST** pass:

1. **Stack Standards Read**: Agent must confirm it read relevant stack files
2. **Code Format**: Code must pass formatter (rustfmt, prettier, black, etc.)
3. **Linter**: Code must pass linter with zero warnings
4. **Type Check**: Code must pass type checker if language supports it
5. **Tests**: All tests must pass
6. **Documentation**: Required documentation must be present
7. **Standards Compliance**: Manual or automated check against stack standards

If any check fails, code **CANNOT** be committed.

## Examples

### Good Practice ✅

**Example 1: Starting Rust Implementation**
```
Agent assigned to implement Rust backend API

Agent process:
1. Reads requirements.md and sees "Language Stack: Rust"
2. **IMMEDIATELY reads .agents/stacks/rust.md** (MANDATORY)
3. Studies coding standards:
   - Must use Result<T, E> for error handling
   - Must use rustfmt for formatting
   - Must use clippy with strict lints
   - Must avoid unwrap() in production code
   - Must write unit tests for all functions
4. Implements code following ALL standards
5. Runs: cargo fmt, cargo clippy, cargo test
6. All checks pass
7. Commits code
8. Discovers a better error handling pattern during implementation
9. Updates .agents/stacks/rust.md Learning Log with new pattern
10. Commits stack file update

✅ Read stack standards BEFORE coding
✅ Followed all documented conventions
✅ Verified compliance with all checks
✅ Updated Learning Log with new learnings
✅ High-quality, consistent code produced
```

**Example 2: Multi-Language Implementation**
```
Agent implementing feature with Rust backend + TypeScript frontend

requirements.md shows:
## Language Stack
- **Rust**: API endpoints (see .agents/stacks/rust.md)
- **TypeScript**: React components (see .agents/stacks/javascript.md)

Agent process:
1. Reads requirements.md, identifies two languages
2. **Reads .agents/stacks/rust.md completely**
3. **Reads .agents/stacks/javascript.md completely**
4. Understands standards for both languages
5. Implements Rust API following Rust standards
6. Implements TypeScript UI following JavaScript standards
7. Runs checks for both: cargo check, prettier, eslint
8. All checks pass for both languages
9. Commits code for both parts
10. Documents integration pattern in both stack files

✅ Read BOTH stack standard files
✅ Applied correct standards to each language
✅ Verified compliance for both languages
✅ Documented cross-language patterns
```

**Example 3: Learning from Mistakes**
```
Agent implementing Python data processing script

Agent process:
1. Reads .agents/stacks/python.md
2. Implements code using mutable global state
3. Code works but colleague reviews and points out anti-pattern
4. Agent realizes mistake:
   - Mutable global state is anti-pattern in Python
   - Should use dependency injection or function parameters
5. **Immediately updates .agents/stacks/python.md Learning Log**:
   ```
   ### 2026-01-11: Avoid Mutable Global State
   **Issue**: Used mutable global state for configuration, causing testing difficulties
   **Learning**: Global state makes code hard to test and reason about
   **Corrective Action**: Refactored to use dependency injection pattern
   **New Standard**: Avoid mutable global state. Use function parameters or DI instead.
   ```
6. Refactors code to follow better pattern
7. Commits corrected code
8. Commits updated stack file

✅ Recognized mistake
✅ Documented learning in stack file
✅ Updated code to follow better pattern
✅ Prevented future agents from making same mistake
```

### Bad Practice ❌

**Example 1: Skipping Stack Standards**
```
Agent assigned to implement JavaScript module

Agent process:
1. Reads requirements.md (sees TypeScript in Language Stack)
2. **SKIPS reading .agents/stacks/javascript.md** (VIOLATION)
3. Starts coding based on "personal knowledge"
4. Uses inconsistent naming (camelCase mixed with snake_case)
5. Doesn't add JSDoc comments
6. Uses `any` types everywhere
7. No error handling
8. Commits code

Result:
❌ CRITICAL VIOLATION: Did not read stack standards
❌ Code violates multiple documented standards
❌ Inconsistent with rest of codebase
❌ Reduced type safety
❌ Missing required documentation
❌ Code must be rejected and rewritten
```

**Example 2: Ignoring Standards**
```
Agent implementing Rust module

Agent process:
1. Reads requirements.md
2. Reads .agents/stacks/rust.md (sees "never use unwrap() in production")
3. Decides "I know better" and uses unwrap() anyway
4. Commits code with unwrap() calls

Result:
❌ CRITICAL VIOLATION: Deliberately ignored documented standards
❌ Code violates explicit prohibition
❌ Introduces panic risk in production
❌ Shows disregard for established rules
❌ Code must be rejected
❌ Violation must be reported
```

**Example 3: Not Documenting Language Stack**
```
Main agent creating specification

Agent process:
1. Has conversation with user about feature
2. Creates requirements.md
3. **FORGETS to add Language Stack section** (VIOLATION)
4. Creates tasks.md without language tools
5. Launches implementation agent

Implementation agent:
- Doesn't know which language to use
- Doesn't know which stack standards to read
- Confused about which conventions to follow
- Wastes time asking for clarification

Result:
❌ VIOLATION: No Language Stack documented
❌ Implementation agent has no direction
❌ Wasted time and confusion
❌ Requirements incomplete
❌ Must go back and fix specification
```

**Example 4: Not Updating Learning Log**
```
Agent implementing feature in Python

Agent process:
1. Reads .agents/stacks/python.md
2. Implements code
3. Makes mistake: uses deprecated Python 2 print statement
4. Realizes mistake and fixes it
5. **Does NOT update Learning Log** (VIOLATION)
6. Moves on to next task

Two weeks later:
- Different agent makes SAME mistake
- No record of previous learning
- Mistake repeated unnecessarily

Result:
❌ VIOLATION: Failed to document learning
❌ Knowledge lost
❌ Same mistake repeated
❌ System doesn't improve
❌ Purpose of Learning Log defeated
```

## Rationale

### Why Language-Specific Standards

1. **Consistency**: All code in a language follows same conventions
2. **Quality**: Standards enforce best practices and quality requirements
3. **Maintainability**: Consistent code is easier to read and modify
4. **Safety**: Standards prevent common bugs and security issues
5. **Onboarding**: New agents can quickly learn project conventions
6. **Professionalism**: Shows care and attention to code quality

### Why Mandatory Reading

1. **Knowledge Transfer**: Ensures agents know the standards
2. **No Assumptions**: Prevents agents from using "default" practices
3. **Explicit Compliance**: Agent can't claim they "didn't know"
4. **Enforceability**: Can verify agent read the standards
5. **Accountability**: Agent responsible for following documented rules

### Why Zero Deviation

1. **Consistency**: Any deviation breaks consistency
2. **Standards Erosion**: Small deviations lead to standard decay
3. **Clear Expectations**: No ambiguity about what's acceptable
4. **Quality Guarantee**: Standards ensure minimum quality level
5. **Trust**: User can trust all code follows same standards

### Why Learning Log

1. **Continuous Improvement**: System gets better over time
2. **Mistake Prevention**: Document mistakes so they aren't repeated
3. **Knowledge Accumulation**: Build institutional knowledge
4. **Pattern Evolution**: Discover and document better patterns
5. **Historical Record**: Understand how standards evolved

### Why Document in Requirements

1. **Visibility**: Everyone knows which languages are used
2. **Planning**: Can plan for language-specific tools and dependencies
3. **Review**: Review agent can verify language documentation
4. **Standards Reference**: Clear pointer to relevant stack files
5. **Completeness**: Requirements are complete when they include language stack

## Integration with Other Rules

### Works With Rule 06 (Specifications and Requirements)
- Language Stack must be documented in requirements.md
- Stack standards must be referenced
- Review agent verifies language documentation
- Implementation agents read stack files before working

### Works With Rule 03 (Work Commit Rules)
- Stack file updates get their own commits
- Learning Log updates are committed when made
- Code commits must pass language-specific checks

### Works With Rule 04 (Agent Orchestration)
- Main agent documents language stack when creating specifications
- Implementation agents read stack files before working
- Agents update stack files when learning occurs

### Works With Rule 05 (Git Auto-Approval and Push)
- Stack file updates are automatically pushed
- Learning Log updates go to repository immediately
- Standards evolve in version control

## Stack File Initialization

When adding a new language to the project:

1. **Create Stack File**
   - Create `.agents/stacks/[language].md`
   - Use template below
   - Fill in all required sections

2. **Document Standards**
   - Research language best practices
   - Document project-specific conventions
   - List required tools and configurations
   - Provide examples

3. **Review and Approve**
   - Have experienced developers review
   - Get user approval for standards
   - Ensure standards are clear and enforceable

4. **Commit and Announce**
   - Commit stack file to repository
   - Update this rule to mention new language
   - Notify team that language is now supported

### Stack File Template

```markdown
# [Language Name] Coding Standards

## Overview
- **Language**: [Name and version]
- **Use Cases**: [When this language is used in project]
- **Official Docs**: [Link to official documentation]

## Setup and Tools

### Required Tools
- [Tool 1]: [Purpose and installation]
- [Tool 2]: [Purpose and installation]

### Configuration Files
- [Config file 1]: [Location and purpose]
- [Config file 2]: [Location and purpose]

## Coding Standards

### Naming Conventions
- **Variables**: [Convention]
- **Functions**: [Convention]
- **Classes**: [Convention]
- **Files**: [Convention]

### Code Organization
- [File structure requirements]
- [Module organization]
- [Import/export conventions]

### Comments and Documentation
- [Comment style requirements]
- [Documentation requirements]
- [When to write comments]

## Best Practices

### Idiomatic Code
- [Language-specific idioms to follow]
- [Patterns to use]
- [Patterns to avoid]

### Error Handling
- [How to handle errors]
- [Error types to use]
- [Logging and reporting]

### Testing
- [Testing requirements]
- [Test organization]
- [Coverage requirements]

### Performance
- [Performance considerations]
- [Optimization guidelines]
- [Profiling approach]

### Security
- [Security best practices]
- [Common vulnerabilities to avoid]
- [Security review requirements]

## Valid Code Requirements

Code is considered valid when:
- [ ] Passes formatter
- [ ] Passes linter with zero warnings
- [ ] Passes type checker (if applicable)
- [ ] All tests pass
- [ ] Documentation is complete
- [ ] Follows all naming conventions
- [ ] Uses proper error handling
- [ ] Has adequate test coverage

## Common Pitfalls

### Pitfall 1: [Name]
**Problem**: [Description]
**Solution**: [How to avoid]

### Pitfall 2: [Name]
**Problem**: [Description]
**Solution**: [How to avoid]

## Examples

### Good Example: [Scenario]
```[language]
[Good code example]
```
**Why This is Good**: [Explanation]

### Bad Example: [Scenario]
```[language]
[Bad code example]
```
**Why This is Bad**: [Explanation]
**How to Fix**: [Corrected version]

## Learning Log

### YYYY-MM-DD: [Title]
**Issue**: [What happened]
**Learning**: [What was learned]
**Corrective Action**: [What was done to fix it]
**New Standard**: [Any new standard that resulted]

---
*Created: [Date]*
*Last Updated: [Date]*
```

## Summary

**Core Principle**: Every programming language has documented standards that agents MUST read and follow without deviation. Standards improve over time through documented learnings.

**Key Points**:
- ✅ Identify and document language stack in requirements.md
- ✅ Create and maintain stack standard files in .agents/stacks/
- ✅ **MANDATORY**: Read stack files before writing any code
- ✅ Follow all standards with zero deviation
- ✅ Update Learning Log when mistakes are made or patterns discovered
- ✅ Run all required checks before committing
- ✅ Keep standards current and accurate
- ❌ **Never write code without reading stack standards**
- ❌ **Never deviate from documented standards**
- ❌ **Never skip updating Learning Log**
- ❌ **Never document language stack in requirements**

**Remember**: Standards exist to ensure quality and consistency. Following them is not optional. Learning from mistakes makes the system better for everyone!

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
