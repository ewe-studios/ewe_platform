# Specifications and Requirements Management

## Purpose
This rule establishes a mandatory requirements-gathering and specification-tracking system that ensures all work begins with a documented conversation between the main agent and user, creating a clear record of requirements and tasks in the `.agents/specifications/` directory.

## Rule

### Requirements-First Development
Before **ANY** work begins on new features, enhancements, or significant changes, the main agent **MUST**:

1. **Engage in a conversation** with the user about requirements
2. **Document the requirements** in a specification directory
3. **Create a task list** for tracking work progress
4. **Have agents read specifications** before starting work
5. **Verify and update status** as work progresses

### No Exceptions
- **NO coding** without documented requirements
- **NO starting work** without a specification
- **NO skipping** the requirements conversation
- This applies to **ALL significant development work**

## Directory Structure

### Overview
```
.agents/
├── specifications/
│   ├── Spec.md                          # Master index of all specifications
│   ├── 01-specification-name/
│   │   ├── requirements.md              # Requirements and conversation summary
│   │   └── tasks.md                     # Task list with checkboxes
│   ├── 02-another-specification/
│   │   ├── requirements.md
│   │   └── tasks.md
│   ├── 03-yet-another-specification/
│   │   ├── requirements.md
│   │   └── tasks.md
│   └── ...
```

### Naming Convention
- Each specification gets its own numbered directory
- Format: `NN-descriptive-name/` where NN is a two-digit number (01, 02, 03, etc.)
- Use dash separators for multi-word names
- Name should clearly describe what the specification is for

**Examples:**
- ✅ `01-build-http-client/`
- ✅ `02-implement-user-authentication/`
- ✅ `03-add-database-migrations/`
- ❌ `http-client/` (missing number prefix)
- ❌ `01_http_client/` (wrong separator)
- ❌ `1-http-client/` (single digit instead of two)

## Spec.md File (Master Index)

### Purpose
The `Spec.md` file serves as the central index and dashboard for all specifications.

### Required Contents
1. **Introduction**: Explanation of what specifications are and how they work
2. **Specifications List**: Links to all specification directories
3. **Status Dashboard**: Breakdown of completed vs pending specifications

### Example Spec.md Structure
```markdown
# Project Specifications

## Overview
This directory contains all project specifications and requirements. Each specification represents a significant feature, enhancement, or change to the project.

## How Specifications Work
1. **Requirements-First**: Before work begins, main agent discusses requirements with user
2. **Documentation**: Requirements and tasks are documented in numbered specification directories
3. **Agent Reading**: Agents MUST read both requirements.md and tasks.md before starting work
4. **Status Verification**: Agents MUST verify completion status by searching the codebase
5. **Task Updates**: Agents MUST update tasks.md as work progresses
6. **Status Accuracy**: Agents MUST ensure status reflects actual implementation

## All Specifications

### [01: Build HTTP Client](./01-build-http-client/)
**Status:** ✅ Completed
**Description:** RESTful HTTP client with request/response handling

### [02: Implement User Authentication](./02-implement-user-authentication/)
**Status:** 🔄 In Progress
**Description:** JWT-based authentication system with role management

### [03: Add Database Migrations](./03-add-database-migrations/)
**Status:** ⏳ Pending
**Description:** Database migration system for schema version control

## Status Dashboard

### Summary
- **Total Specifications:** 3
- **Completed:** 1 (33%)
- **In Progress:** 1 (33%)
- **Pending:** 1 (33%)

### Completed ✅
- 01: Build HTTP Client

### In Progress 🔄
- 02: Implement User Authentication

### Pending ⏳
- 03: Add Database Migrations

---
*Last updated: 2026-01-11*
```

## requirements.md File

### Purpose
Documents the detailed requirements from the conversation between main agent and user.

### File Structure
```markdown
---
description: Brief one-sentence description of what this specification is for
status: completed | uncompleted
---

# [Specification Name] - Requirements

## Overview
Brief summary of what this specification covers and why it's needed.

## Requirements Conversation Summary

### User Request
[Summary of what the user initially requested]

### Clarifying Questions
[Questions the agent asked to understand requirements better]

### User Responses
[User's answers and additional context provided]

### Final Requirements Agreement
[What was agreed upon as the final set of requirements]

## Detailed Requirements

### Functional Requirements
1. [Requirement 1]
2. [Requirement 2]
3. [Requirement 3]

### Non-Functional Requirements
1. [Performance requirements]
2. [Security requirements]
3. [Compatibility requirements]

### Technical Specifications
- **Technology Stack:** [Technologies to be used]
- **Dependencies:** [Required libraries/tools]
- **Integration Points:** [How this integrates with existing code]

### Success Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Important Notes for Agents

### Before Starting Work
- **MUST READ** both this requirements.md and tasks.md files
- **MUST VERIFY** completion status by searching the codebase
- **MUST UPDATE** tasks.md to reflect actual implementation status
- **MUST ADD** new tasks to tasks.md BEFORE starting work on them

### Verification Requirements
Agents cannot rely solely on the status field or task checkboxes. They **MUST**:
1. Search the codebase for relevant files and implementations
2. Verify that code actually exists and works as specified
3. Update task status based on actual findings, not assumptions
4. Mark tasks as completed only when fully verified in codebase

### During Work
- Update tasks.md as you complete each task
- Add new tasks if you discover additional work needed
- Keep frontmatter counts accurate in tasks.md
- Update tools list as new tools are used

---
*Created: [Date]*
*Last Updated: [Date]*
```

### Example requirements.md
```markdown
---
description: Build RESTful HTTP client with request/response handling and error management
status: completed
---

# HTTP Client - Requirements

## Overview
This specification covers the implementation of a robust HTTP client library for making RESTful API calls with comprehensive error handling, request/response transformation, and middleware support.

## Requirements Conversation Summary

### User Request
User requested a HTTP client that can handle common REST operations with support for authentication headers, request retries, and response parsing.

### Clarifying Questions
Agent asked:
- What HTTP methods need to be supported?
- Should it include built-in authentication handling?
- What level of error handling is required?
- Should it support request/response interceptors?

### User Responses
User confirmed:
- Support for GET, POST, PUT, PATCH, DELETE methods
- Built-in support for Bearer token authentication
- Automatic retry on network failures (configurable)
- Request and response interceptor middleware
- TypeScript support with full type definitions

### Final Requirements Agreement
Build a TypeScript HTTP client class with method chaining, middleware support, automatic retries, and comprehensive error handling.

## Detailed Requirements

### Functional Requirements
1. Support all standard HTTP methods (GET, POST, PUT, PATCH, DELETE)
2. Automatic JSON request/response parsing
3. Configurable base URL and default headers
4. Request and response interceptor middleware
5. Automatic retry on network failures (with configurable attempts)
6. Bearer token authentication helper methods
7. Query parameter building and encoding
8. Custom error classes for different failure types

### Non-Functional Requirements
1. **Performance:** Minimal overhead over native fetch
2. **Security:** No credential leakage in error messages
3. **Compatibility:** Works in Node.js 18+ and modern browsers
4. **Type Safety:** Full TypeScript type definitions

### Technical Specifications
- **Technology Stack:** TypeScript, native fetch API
- **Dependencies:** None (uses built-in fetch)
- **Integration Points:** Used throughout application for all API calls

### Success Criteria
- [x] All HTTP methods implemented and tested
- [x] Middleware system works correctly
- [x] Retry logic handles failures properly
- [x] TypeScript types are accurate and helpful
- [x] Error handling covers all edge cases
- [x] Documentation is complete

## Important Notes for Agents

### Before Starting Work
- **MUST READ** both this requirements.md and tasks.md files
- **MUST VERIFY** completion status by searching the codebase
- **MUST UPDATE** tasks.md to reflect actual implementation status
- **MUST ADD** new tasks to tasks.md BEFORE starting work on them

### Verification Requirements
Search for:
- `src/http-client.ts` or similar file
- Test files for HTTP client
- Integration usage in other parts of codebase
- Documentation files

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
```

## tasks.md File

### Purpose
Tracks all tasks required to complete the specification using markdown checkboxes.

### File Structure with Frontmatter
```markdown
---
completed: 5
uncompleted: 3
tools:
  - TypeScript
  - Jest
  - ESLint
  - Prettier
---

# [Specification Name] - Tasks

## Task List

### Implementation Tasks
- [x] Create base HttpClient class
- [x] Implement GET method
- [x] Implement POST method
- [x] Implement PUT method
- [x] Implement DELETE method
- [ ] Add request interceptor middleware
- [ ] Add response interceptor middleware
- [ ] Implement retry logic

### Testing Tasks
- [x] Write unit tests for GET method
- [ ] Write unit tests for POST method
- [ ] Write integration tests

### Documentation Tasks
- [x] Write API documentation
- [ ] Add usage examples
- [ ] Create migration guide

## Notes
- Retry logic needs configuration for max attempts and backoff strategy
- Middleware system should support async middleware functions
- Consider adding request timeout configuration

---
*Last Updated: 2026-01-11*
```

### Frontmatter Fields

#### Required Fields
- **completed**: Total count of completed tasks (checkbox count with `[x]`)
- **uncompleted**: Total count of uncompleted tasks (checkbox count with `[ ]`)
- **tools**: List of tools, skills, and MCP tools required or used

#### Counting Rules
- Count must match actual number of checkboxes in the file
- Update counts every time task status changes
- Use search/count to verify accuracy

### Checkbox Format
- Uncompleted task: `- [ ] Task description`
- Completed task: `- [x] Task description`
- Use consistent spacing (dash, space, bracket, space/x, bracket, space, description)

### Task Management Rules
1. **Before starting work**: Add task to tasks.md
2. **During work**: Keep task as `[ ]` uncompleted
3. **After completing**: Change to `[x]` completed
4. **Update frontmatter**: Adjust completed/uncompleted counts
5. **Update tools**: Add any new tools used

## Workflow

### Complete Requirements-to-Implementation Workflow

```
1. User Requests Feature
   ↓
2. Main Agent Conversation with User
   ├─ Ask clarifying questions
   ├─ Understand full requirements
   ├─ Confirm technical approach
   └─ Get user agreement
   ↓
3. Create Specification Directory
   ├─ Determine next number (e.g., 04)
   ├─ Create directory: .agents/specifications/04-feature-name/
   └─ Create both requirements.md and tasks.md files
   ↓
4. Document Requirements
   ├─ Fill in requirements.md frontmatter
   ├─ Write conversation summary
   ├─ List detailed requirements
   └─ Include agent notes
   ↓
5. Create Task List
   ├─ Fill in tasks.md frontmatter
   ├─ Break down work into tasks
   ├─ List all tools needed
   └─ All tasks start as [ ] uncompleted
   ↓
6. Update Spec.md Master Index
   ├─ Add new specification to list
   ├─ Update status dashboard counts
   └─ Link to new specification directory
   ↓
7. Commit Specification Files
   ├─ git add .agents/specifications/
   ├─ git commit (following Rule 03)
   ├─ git push (following Rule 05)
   └─ Verify success
   ↓
8. Launch Specialized Agents (Rule 04)
   ├─ Agents MUST read requirements.md
   ├─ Agents MUST read tasks.md
   ├─ Agents MUST verify status by searching codebase
   └─ Agents work on tasks
   ↓
9. Agent Updates During Work
   ├─ Add new tasks BEFORE starting work on them
   ├─ Update task checkboxes as work completes
   ├─ Update frontmatter counts
   ├─ Update tools list
   └─ Commit changes to tasks.md after updates
   ↓
10. Verification and Completion
    ├─ Agent searches codebase to verify implementation
    ├─ Agent updates tasks.md with actual status
    ├─ Agent updates requirements.md status if all done
    ├─ Main agent updates Spec.md master index
    └─ Final commit and push
```

## Verification Requirements

### Critical: Verify Actual Implementation
Agents **MUST NOT** trust the status field or task checkboxes blindly. Instead, agents **MUST**:

1. **Search the Codebase**
   - Use Glob to find relevant files
   - Use Grep to search for specific implementations
   - Read key files to verify functionality exists

2. **Verify Implementation Quality**
   - Code exists and is not just a stub
   - Tests are present and passing
   - Documentation is complete
   - Integration points work correctly

3. **Update Status Accordingly**
   - Mark tasks as `[x]` only if truly implemented
   - Change tasks back to `[ ]` if implementation is missing or incomplete
   - Update frontmatter counts to match reality
   - Update requirements.md status field if specification is complete

4. **Report Discrepancies**
   - If status says "completed" but implementation is missing, report this
   - If tasks are marked done but code doesn't exist, correct it
   - User should know if specifications are inaccurate

### Example Verification Process
```
Agent reads specification 02-user-authentication/tasks.md
Task says: "[x] Implement JWT token generation"

Agent verification steps:
1. Glob for: "**/*auth*.ts" or "**/*jwt*.ts"
2. Grep for: "generateToken|createToken|signJWT"
3. Read found files to verify implementation
4. Check tests exist and pass
5. If implementation exists and works:
   → Keep as [x] completed
6. If implementation missing or broken:
   → Change to [ ] uncompleted
   → Update frontmatter count
   → Add note about what needs fixing
```

## Examples

### Good Practice ✅

**Example 1: Starting New Specification**
```
User: "I need a caching layer for API responses"

Main Agent:
1. "Let me clarify some requirements..."
   - What caching strategy? (LRU, TTL-based, etc.)
   - Where should cache be stored? (memory, Redis, etc.)
   - What should be cached? (all responses, specific endpoints?)
   - What's the invalidation strategy?

2. User provides answers

3. Agent creates:
   - .agents/specifications/05-api-response-caching/
   - requirements.md with full conversation summary
   - tasks.md with all identified tasks
   - Updates Spec.md with new specification

4. Agent commits specification files

5. Agent launches specialized agents to implement
   - Each agent reads requirements.md and tasks.md first
   - Agents verify status by searching codebase
   - Agents update tasks.md as they work

✅ Requirements documented before work began
✅ Full conversation captured
✅ Clear task list created
✅ Agents have clear direction
```

**Example 2: Agent Verifying Status**
```
Agent assigned to work on specification 03-database-migrations

Agent process:
1. Reads requirements.md
2. Reads tasks.md (shows some tasks as [x] completed)
3. Searches codebase:
   - Globs for "**/migrations/*.ts"
   - Greps for "migrate|migration|schema"
   - Reads migration files to verify implementation
4. Finds that 2 tasks marked as done are actually not implemented
5. Updates tasks.md:
   - Changes those tasks back to [ ]
   - Updates frontmatter: completed: 5 → 3, uncompleted: 2 → 4
   - Adds note explaining discrepancy
6. Commits the correction
7. Proceeds with actual implementation

✅ Verified actual status before trusting checkboxes
✅ Corrected inaccurate status
✅ User gets accurate picture of progress
```

**Example 3: Adding Tasks During Work**
```
Agent working on specification 04-file-upload-system

Agent discovers additional work needed:
1. Opens tasks.md
2. Adds new tasks BEFORE starting work:
   - [ ] Add file size validation
   - [ ] Implement virus scanning integration
   - [ ] Add progress tracking events
3. Updates frontmatter: uncompleted: 8 → 11
4. Commits tasks.md update
5. Begins implementation of new tasks

✅ Added tasks before starting work
✅ Updated counts immediately
✅ Clear record of scope expansion
```

**Example 4: Updating Spec.md Dashboard**
```
Agent completes specification 02-user-authentication

Agent process:
1. Verifies all tasks are truly complete (by searching codebase)
2. Updates requirements.md: status: uncompleted → completed
3. Updates Spec.md:
   - Changes status to ✅ Completed
   - Updates dashboard counts
   - Moves from "In Progress" to "Completed" section
4. Commits all changes
5. Reports completion to user

✅ Verified completion before marking done
✅ Updated all relevant files
✅ Dashboard reflects accurate status
```

### Bad Practice ❌

**Example 1: Starting Work Without Requirements**
```
User: "Add user authentication"

Main Agent:
Launches agents immediately to implement authentication

❌ No requirements conversation
❌ No specification directory created
❌ No documented requirements
❌ Agents don't know what to implement
❌ No task tracking
❌ User expectations may not be met
```

**Example 2: Trusting Status Without Verification**
```
Agent reads specification 05-api-caching
tasks.md shows: "[x] Implement Redis cache adapter"

Agent assumes it's done and moves to next task

But actually:
- No Redis adapter exists in codebase
- Previous agent marked it done incorrectly
- Implementation is missing

❌ Didn't verify actual implementation
❌ Assumed checkbox was accurate
❌ Failed to search codebase
❌ Left incomplete work as "completed"
```

**Example 3: Not Adding Tasks Before Work**
```
Agent working on specification 06-payment-integration

Agent starts implementing Stripe integration without adding task

Later marks task as complete in tasks.md retroactively

❌ Started work without task documented
❌ No record of work scope before implementation
❌ Tasks should be added BEFORE work begins
```

**Example 4: Not Updating Counts**
```
Agent completes 3 tasks in specification 07-email-system

Agent updates checkboxes:
- [ ] Task 1 → [x] Task 1
- [ ] Task 2 → [x] Task 2
- [ ] Task 3 → [x] Task 3

But doesn't update frontmatter counts

Frontmatter still shows:
---
completed: 2
uncompleted: 8
---

❌ Counts don't match actual checkboxes
❌ Dashboard will show wrong progress
❌ Frontmatter must be updated with checkboxes
```

**Example 5: Vague Requirements Documentation**
```
requirements.md content:
---
description: Add authentication
status: uncompleted
---

# Authentication

User wants authentication.

Will implement JWT.

❌ No conversation summary
❌ No detailed requirements
❌ No technical specifications
❌ No success criteria
❌ No agent notes
❌ Too vague to be useful
```

## Rationale

### Why Requirements-First Development
1. **Clear Direction**: Agents know exactly what to implement
2. **User Alignment**: Ensures work meets user expectations
3. **Scope Control**: Prevents scope creep and unnecessary work
4. **Better Planning**: Can estimate effort and identify dependencies
5. **Documentation**: Creates permanent record of decisions
6. **Onboarding**: New agents/developers can understand project evolution

### Why Conversation Documentation
1. **Context Preservation**: Future agents understand the "why" not just "what"
2. **Decision Record**: Captures reasoning behind technical choices
3. **Clarification History**: Shows what questions were asked and answered
4. **Requirement Changes**: Can see how requirements evolved
5. **Knowledge Transfer**: Helps humans understand agent's understanding

### Why Task Tracking
1. **Progress Visibility**: User can see what's done and what remains
2. **Work Planning**: Agents can pick up where others left off
3. **Scope Management**: Clear list of what's in and out of scope
4. **Accountability**: Clear record of completed work
5. **Estimation**: Can gauge project completion percentage

### Why Verification is Critical
1. **Accuracy**: Status must reflect reality, not assumptions
2. **Trust**: User can rely on status information being correct
3. **Quality**: Ensures work is actually done, not just marked done
4. **Debugging**: Prevents confusion about what's implemented
5. **Handoffs**: Next agent gets accurate picture of state

### Why Frontmatter in Files
1. **Quick Reading**: Can see status without reading full file
2. **Machine Readable**: Tools can parse frontmatter for dashboards
3. **Metadata Separation**: Keeps metadata distinct from content
4. **Standard Format**: Uses established YAML frontmatter convention
5. **Efficiency**: Agents can scan multiple files quickly

### Why Master Index (Spec.md)
1. **Central Dashboard**: Single place to see all specifications
2. **Quick Navigation**: Links to all specification directories
3. **Status Overview**: Bird's-eye view of project progress
4. **Discoverability**: Easy to find specifications
5. **Progress Tracking**: User can monitor overall completion

## Enforcement

### Mandatory Compliance
All agents **MUST**:
- Never begin significant work without documented requirements
- Create specification directory before starting implementation
- Document requirements conversation thoroughly
- Create comprehensive task list before work begins
- Read both requirements.md and tasks.md before starting work
- Verify status by searching codebase, not trusting checkboxes
- Update tasks.md as work progresses
- Update frontmatter counts whenever task status changes
- Add new tasks BEFORE starting work on them
- Commit specification changes following Rule 03 and Rule 05

### Violations

Any of the following constitutes a serious violation:
- Starting implementation without documented requirements
- Not creating specification directory and files
- Skipping requirements conversation with user
- Trusting task status without verifying in codebase
- Not updating tasks.md during work
- Not updating frontmatter counts
- Starting work on tasks not yet added to tasks.md
- Incomplete or vague requirements documentation
- Not updating Spec.md master index

### User Impact
Violations have serious consequences:
- **User frustration**: Work doesn't meet expectations
- **Wasted effort**: Implementation may be wrong or unnecessary
- **Lost context**: Future agents don't understand requirements
- **False progress**: Status shows completion when work is incomplete
- **Confusion**: User can't understand what's been done
- **Rework**: May need to redo work due to misunderstanding

**THE USER WILL BE UPSET** if work proceeds without proper requirements documentation and status verification!

### Corrective Action

When a violation occurs:
1. **Stop immediately** if work has started without requirements
2. **Create specification** if missing
3. **Document requirements** by having conversation with user
4. **Create task list** before proceeding
5. **Verify status** by searching codebase if relying on checkboxes
6. **Update files** to reflect accurate status
7. **Commit changes** following proper git workflow

## Special Cases

### Small Bug Fixes
Very small bug fixes (single line changes) may not require full specification:
- Use judgment: if it takes longer to document than fix, proceed with fix
- Still commit with detailed message per Rule 03
- Consider adding to existing specification if related to one

### Urgent Hotfixes
For critical production issues:
- Fix the issue immediately
- Document requirements retroactively
- Create specification documenting what was done and why

### Research Tasks
For research/exploration tasks without implementation:
- Create specification with research questions
- Document findings in requirements.md
- Use tasks.md to track research activities

### Documentation-Only Changes
For pure documentation updates:
- May not need full specification
- Use judgment based on scope
- Major documentation overhauls should get specification

## Integration with Other Rules

### Works With Rule 03 (Work Commit Rules)
- Specification files follow commit-after-every-change rule
- Each specification update gets its own commit
- Commit messages explain what was added/changed in specifications

### Works With Rule 04 (Agent Orchestration)
- Main agent creates specifications before launching specialized agents
- Specialized agents MUST read specifications before working
- Agents report back with task completion status
- Main agent updates Spec.md based on agent reports

### Works With Rule 05 (Git Auto-Approval and Push)
- Specification changes are automatically pushed
- No approval needed for specification updates
- Each specification commit is pushed immediately

## Summary

**Core Principle**: Never start significant work without documented requirements and a clear task list. Always verify status by checking actual implementation, never trust checkboxes blindly.

**Key Points**:
- ✅ Requirements conversation comes first
- ✅ Document everything in specification directory
- ✅ Create comprehensive task list before work begins
- ✅ Agents read specifications before working
- ✅ Verify status by searching codebase
- ✅ Update tasks.md as work progresses
- ✅ Keep Spec.md master index current
- ❌ Never start work without documented requirements
- ❌ Never trust status without verification
- ❌ Never add tasks retroactively
- ❌ Never skip updating frontmatter counts

**Remember**: The user will be upset if work proceeds without proper requirements or if status information is inaccurate!

---
*Created: 2026-01-11*
