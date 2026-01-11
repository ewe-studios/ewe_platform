# Work Commit Rules

## Purpose
This rule establishes mandatory version control practices requiring immediate commits after every change during development work.

## Rule

### Immediate Commit Requirement
After **EVERY** change or modification to any file in the codebase, agents **MUST**:

1. **Stage the modified files** using `git add [relevant files]`
2. **Create a commit immediately** using `git commit -m "[message]"`
3. **Verify the commit succeeded** before proceeding to the next task
4. **Include co-authorship attribution** in every commit message

### No Exceptions
- Commits are **MANDATORY** after each change
- **NO batching** of commits at the end of work
- **NO skipping** commits for "small changes"
- **NO deferring** commits until "later"
- This applies to **ALL file types**: code, configuration, documentation, tests, etc.

### Commit Message Format
Every commit message **MUST** include:

1. **Brief summary line** (50 characters or less) describing what was done
2. **Blank line**
3. **Detailed explanation** of what was changed and why
4. **Bullet-point summary** of specific changes made
5. **Blank line**
6. **Co-authorship attribution**: `Co-Authored-By: Claude <noreply@anthropic.com>`

**Template:**
```
Brief summary of change

Detailed explanation of what was changed and why this change
was necessary. Explain the context and reasoning behind the
modification.

Changes made:
- Specific change 1
- Specific change 2
- Specific change 3

Co-Authored-By: Claude <noreply@anthropic.com>
```

### Commit Verification
After each commit, agents **MUST**:
1. Run `git status` to confirm working directory is clean
2. Check that the commit was created successfully
3. Verify all intended files were included in the commit
4. **ONLY THEN** proceed to the next task or change

## Workflow

### Single Change Workflow
```
1. Make a change to file(s)
   ↓
2. git add [files]
   ↓
3. git commit -m "[detailed message with bullets and co-author]"
   ↓
4. git status (verify success)
   ↓
5. Proceed to next change (if any)
```

### Multiple Changes Workflow
```
Change 1 → git add → git commit → verify →
Change 2 → git add → git commit → verify →
Change 3 → git add → git commit → verify →
... and so on
```

**Never:**
```
Change 1 → Change 2 → Change 3 → git add → git commit ❌
```

## Examples

### Good Practice ✅

**Example 1: Adding a new feature**
```bash
# After adding authentication middleware
git add src/middleware/auth.js
git commit -m "$(cat <<'EOF'
Add authentication middleware for API routes

Implemented JWT-based authentication middleware to secure API
endpoints. This middleware validates JWT tokens and attaches
user information to the request object.

Changes made:
- Created auth.js middleware with token validation
- Added JWT verification using jsonwebtoken library
- Implemented error handling for invalid/expired tokens
- Added user object attachment to req.user

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

**Example 2: Fixing a bug**
```bash
# After fixing validation error
git add src/validators/user-validator.js
git commit -m "$(cat <<'EOF'
Fix email validation regex to support plus addressing

Fixed bug where email addresses with plus signs (user+tag@domain.com)
were incorrectly rejected by the validation logic.

Changes made:
- Updated email regex pattern to include plus sign
- Added test cases for plus addressing
- Updated validation error messages

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

**Example 3: Updating documentation**
```bash
# After updating README
git add README.md
git commit -m "$(cat <<'EOF'
Update installation instructions for Node 18+

Updated documentation to reflect new Node.js version requirement
and simplified installation steps based on user feedback.

Changes made:
- Changed minimum Node.js version to 18.0.0
- Removed deprecated npm install flags
- Added troubleshooting section for common issues
- Fixed formatting in code examples

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

**Example 4: Multiple files in single logical change**
```bash
# After adding new API endpoint
git add src/routes/users.js src/controllers/user-controller.js tests/user-routes.test.js
git commit -m "$(cat <<'EOF'
Add GET /users/:id endpoint for user profile retrieval

Implemented new endpoint to fetch individual user profiles by ID,
including associated controller logic and comprehensive tests.

Changes made:
- Added GET /users/:id route in users.js
- Implemented getUserById controller method
- Added validation for user ID parameter
- Created test suite with 8 test cases
- Added error handling for non-existent users

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

### Bad Practice ❌

**Example 1: Batching multiple unrelated changes**
```bash
# Made changes to auth.js, user-validator.js, and README.md
git add .
git commit -m "Updated files"

❌ Multiple unrelated changes in one commit
❌ Non-descriptive commit message
❌ No detailed explanation
❌ No bullet points
❌ Missing co-authorship
```

**Example 2: Making multiple changes before committing**
```bash
# Changed file A
# Changed file B
# Changed file C
git add file-a.js file-b.js file-c.js
git commit -m "Multiple updates"

❌ Should have committed after each change
❌ Batched commits instead of immediate commits
```

**Example 3: Vague commit message**
```bash
git add src/api.js
git commit -m "Fix bug"

❌ No detailed explanation
❌ No bullet points
❌ Missing co-authorship
❌ Too vague - which bug?
```

**Example 4: Skipping verification**
```bash
git add config.js
git commit -m "Update config"
# Immediately proceeds without git status check

❌ Did not verify commit success
❌ Did not check if working directory is clean
```

**Example 5: Missing co-authorship**
```bash
git add utils.js
git commit -m "$(cat <<'EOF'
Add utility function for date formatting

Added formatDate function to handle various date format conversions.

Changes made:
- Created formatDate utility function
- Added support for ISO, US, and EU formats
- Implemented timezone handling
EOF
)"

❌ Missing "Co-Authored-By: Claude <noreply@anthropic.com>"
```

## Rationale

### Why Immediate Commits Matter
1. **Atomic Changes**: Each commit represents a single, logical unit of work
2. **Clear History**: Makes it easy to understand what changed and why
3. **Easier Rollback**: Can revert specific changes without affecting other work
4. **Better Collaboration**: Other agents/developers can see progress in real-time
5. **Accountability**: Clear attribution of who made what changes
6. **Debugging**: Easier to identify when and where bugs were introduced
7. **Code Review**: Smaller, focused commits are easier to review

### Why Detailed Messages Matter
1. **Context**: Future readers understand the reasoning behind changes
2. **Documentation**: Commit history serves as a development log
3. **Searchability**: Detailed messages make it easier to find specific changes
4. **Knowledge Transfer**: Helps new team members understand evolution of code

### Why Co-Authorship Matters
1. **Transparency**: Clear indication that AI assisted with the change
2. **Attribution**: Proper credit for collaborative work
3. **Tracking**: Helps identify AI-generated code for review purposes
4. **Standards**: Maintains ethical AI usage practices

## Enforcement

### Violations
Any of the following constitutes a violation:
- Making multiple changes before committing
- Batching commits at the end of work
- Using vague or non-descriptive commit messages
- Omitting detailed explanations or bullet points
- Missing co-authorship attribution
- Failing to verify commit success
- Skipping commits for "small" changes

### Corrective Action
When a violation occurs:
1. **Stop immediately** and do not proceed with further changes
2. **Create proper commits** for any uncommitted changes
3. **Follow the correct format** for commit messages
4. **Verify each commit** before proceeding
5. **Report the violation** to maintain awareness

### Self-Enforcement
All agents **MUST**:
- Treat this rule as non-negotiable
- Build commit steps into their workflow
- Default to "commit after every change" behavior
- When in doubt, commit rather than batch

## Special Cases

### Configuration Changes
Even simple configuration changes **MUST** be committed immediately:
```bash
# After changing a single config value
git add .env.example
git commit -m "$(cat <<'EOF'
Update default API timeout to 30 seconds

Changed the default API timeout from 10 to 30 seconds to
accommodate slower network conditions reported by users.

Changes made:
- Updated API_TIMEOUT value in .env.example
- Increased from 10000ms to 30000ms

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

### Test Files
Test file changes follow the same rules:
```bash
# After adding a test case
git add tests/auth.test.js
git commit -m "$(cat <<'EOF'
Add test case for expired token handling

Added test to verify proper error handling when authentication
is attempted with an expired JWT token.

Changes made:
- Added test case for expired token scenario
- Mocked Date.now() to simulate token expiration
- Verified 401 status code is returned
- Checked error message matches expected format

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

### Refactoring
Refactoring commits should explain both what changed and why:
```bash
# After refactoring a function
git add src/services/payment-service.js
git commit -m "$(cat <<'EOF'
Refactor payment processing to use async/await

Modernized payment processing code by converting Promise chains
to async/await syntax for improved readability and error handling.

Changes made:
- Converted processPayment() to async function
- Replaced .then() chains with await statements
- Consolidated error handling into try/catch block
- Removed nested Promise callbacks

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status
```

## Integration with Other Rules

This rule works in conjunction with:
- **Agent Orchestration (Rule 03)**: Each specialized agent must commit their changes
- **Directory Policy (Rule 02)**: Commits apply to files in all locations
- **Naming Conventions (Rule 01)**: Commits apply to rule files and all other files

---
*Created: 2026-01-11*
