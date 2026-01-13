# Work Commit and Push Rules

## Purpose
This rule establishes mandatory version control practices requiring immediate commits after every change during development work, followed by automatic push to remote repository, eliminating manual approval steps while maintaining strict safety guarantees.

## Core Principles

### 1. Immediate Commit Requirement
After **EVERY** change or modification to any file in the codebase, agents **MUST** commit immediately.

### 2. Automatic Push Requirement
After **EVERY** successful commit, agents **MUST** automatically push to remote without user confirmation.

### 3. Safety First
Only safe, non-destructive git operations are allowed. Destructive operations are **ABSOLUTELY FORBIDDEN**.

## Rule: Immediate Commit and Automatic Push

### Complete Workflow (Code Changes)
```
1. Implementation agent completes code changes
   ↓
2. Reports to Main Agent (never commits directly)
   ↓
3. Main Agent spawns Verification Agent (Rule 05)
   ↓
4. Verification Agent runs ALL checks
   ↓
5. IF ALL PASS:
   ↓
6. Main Agent: git add [files]
   ↓
7. Main Agent: git commit -m "[message with verification status]"
   ↓
8. Main Agent: git status (verify commit succeeded)
   ↓
9. Main Agent: git push (automatic - no approval needed)
   ↓
10. Verify push succeeded
   ↓
11. Proceed to next task

IF ANY FAIL: Main Agent creates urgent task, does NOT commit
```

### Complete Workflow (Non-Code Changes)
```
1. Make changes to file(s) (docs, config, etc.)
   ↓
2. git add [files]
   ↓
3. git commit -m "[message with co-author]"
   ↓
4. git status (verify commit succeeded)
   ↓
5. git push (automatic - no approval needed)
   ↓
6. Verify push succeeded
   ↓
7. Proceed to next task
```

### No Exceptions
- **NO batching** of commits at the end of work
- **NO skipping** commits for "small changes"
- **NO deferring** commits until "later"
- **NO asking** for permission to commit or push
- **NO manual approval** steps in the workflow
- **ALWAYS push** after successful commit
- This applies to **ALL file types**: code, configuration, documentation, tests, etc.

## Commit Message Format (MANDATORY)

Every commit message **MUST** include:

1. **Brief summary line** (50 characters or less)
2. **Blank line**
3. **Detailed explanation** of what was changed and why
4. **Bullet-point summary** of specific changes
5. **Blank line**
6. **Verification status** (if code changes were made)
7. **Blank line** (if verification section included)
8. **Co-authorship attribution**: `Co-Authored-By: Claude <noreply@anthropic.com>`

### Template for Non-Code Changes
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

### Template for Code Changes (with Verification)
```
Brief summary of change

Detailed explanation of what was changed and why this change
was necessary. Explain the context and reasoning behind the
modification.

Changes made:
- Specific change 1
- Specific change 2
- Specific change 3

Verified by [Language] Verification Agent: All checks passed
- Format: PASS
- Lint: PASS
- Type Check: PASS (if applicable)
- Tests: [N]/[N] PASS
- Build: PASS
- Coverage: [N]%

Co-Authored-By: Claude <noreply@anthropic.com>
```

## Code Verification Before Commit (MANDATORY)

**CRITICAL**: When committing code changes (not documentation or configuration), agents **MUST**:

1. **NEVER commit directly** after implementation
2. **ALWAYS delegate to verification agent first** (see Rule 05)
3. **WAIT for verification results** before committing
4. **ONLY commit if ALL verifications PASS**
5. **INCLUDE verification status** in commit message
6. **AUTOMATICALLY push** after successful commit

See **Rule 05 (Coding Practice and Agent Orchestration)** for complete verification workflow details.

## Safety Requirements - Destructive Operations FORBIDDEN

To ensure automatic approval is safe, the following operations are **ABSOLUTELY FORBIDDEN**:

### ❌ FORBIDDEN Operations
- `git push --force` or `git push -f` (force push)
- `git push --force-with-lease` (force push variant)
- `git reset --hard` (hard reset)
- `git rebase -i` (interactive rebase)
- `git filter-branch` (history rewriting)
- `git reflog expire` (reflog deletion)
- `git gc --prune=now` (aggressive garbage collection)
- `git branch -D` (force delete branch)
- `git reset --hard HEAD~N` (discarding commits)
- `git commit --amend` (unless specific conditions met per Rule 03)
- Any command with `--force` flag
- Any history-rewriting operations
- Any operations that could destroy data or corrupt git history

### ✅ ALLOWED Operations
- `git add [files]` (stage files)
- `git commit -m "[message]"` (create commit)
- `git status` (check status)
- `git push` (push to remote - standard push only)
- `git pull` (pull from remote)
- `git fetch` (fetch from remote)
- `git branch` (list or create branches)
- `git checkout -b [branch]` (create new branch)
- `git checkout [branch]` (switch branches)
- `git log` (view history)
- `git diff` (view differences)
- `git stash` (stash changes)
- `git stash pop` (apply stashed changes)
- `git merge` (merge branches - non-force)
- `git branch -d` (delete merged branch - soft delete)

### Safety Guarantees

This automatic system is safe because:

1. **Non-destructive operations only**: All allowed operations preserve history and data
2. **Standard push only**: Regular `git push` will fail if conflicts exist, forcing proper resolution
3. **No force operations**: Cannot overwrite remote history or other developers' work
4. **Verification required**: Code commits only happen after all checks pass
5. **Atomic commits**: Each commit is verified before pushing
6. **Recoverable**: All operations can be undone using standard git recovery methods

## Examples

### Good Practice ✅

**Example 1: Adding a new feature with verification**
```bash
# After implementation and successful verification
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
- Wrote comprehensive test suite

Verified by JavaScript Verification Agent: All checks passed
- Format: PASS (prettier)
- Lint: PASS (eslint, 0 warnings)
- Type Check: PASS (tsc)
- Tests: 12/12 PASS
- Build: PASS
- Coverage: 94%

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
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
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
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
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
```

**Example 4: Rust code change with verification**
```bash
# After implementing Rust module and successful verification
git add src/auth/token.rs tests/token_tests.rs
git commit -m "$(cat <<'EOF'
Implement JWT token generation and validation

Created robust JWT token handling module with generation,
validation, and refresh token support.

Changes made:
- Implemented TokenManager struct with key management
- Added generate_token() with configurable expiration
- Added validate_token() with signature verification
- Implemented refresh token rotation
- Added comprehensive test suite with edge cases

Verified by Rust Verification Agent: All checks passed
- Format: PASS (rustfmt)
- Lint: PASS (clippy, 0 warnings)
- Tests: 23/23 PASS
- Build: PASS (debug and release)
- Doc: PASS (cargo doc)
- Security: PASS (cargo audit)

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
```

### Bad Practice ❌

**Example 1: Batching multiple unrelated changes**
```bash
# Made changes to auth.js, user-validator.js, and README.md
git add .
git commit -m "Updated files"
git push

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
git push

❌ Should have committed after each change
❌ Batched commits instead of immediate commits
```

**Example 3: Asking for approval**
```bash
git add src/feature.js
git commit -m "Add feature"
# Agent asks: "Should I push this to remote?"

❌ Never ask for approval to push
❌ Push should be automatic after commit
```

**Example 4: Not pushing automatically**
```bash
git add src/feature.js
git commit -m "Add feature"
git status
# Agent stops here without pushing

❌ Must automatically push after commit
❌ Workflow is incomplete without push
```

**Example 5: Using force push**
```bash
git add src/feature.js
git commit -m "Add feature"
git push --force

❌ Force push is absolutely forbidden
❌ Destructive operation violates safety requirements
```

**Example 6: Committing code without verification (CRITICAL VIOLATION)**
```bash
# Implementation agent completes work
git add src/payment/processor.js
git commit -m "$(cat <<'EOF'
Add payment processing module

Implemented payment processing with Stripe integration.

Changes made:
- Created PaymentProcessor class
- Added Stripe API integration
- Implemented error handling

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git push

❌ CRITICAL VIOLATION: Code committed without verification
❌ No verification agent was delegated to
❌ Tests might be failing
❌ Code might not compile
❌ This violates Rule 05 (ZERO TOLERANCE)
```

## Main Agent Push Verification Responsibility (CRITICAL)

**MANDATORY**: The Main Agent **MUST** verify that sub-agents have pushed after committing.

### Main Agent Responsibilities

**When Main Agent commits code** (after verification passes per Rule 05):
1. ✅ Execute commit and push:
   ```bash
   git add [files]
   git commit -m "[message]"
   git status  # Verify commit succeeded
   git push    # Push to remote
   ```
2. ✅ Verify push succeeded (check for errors)
3. ✅ Confirm push in report to user

**When Main Agent receives completion report from sub-agent**:
1. ✅ Check if sub-agent pushed to remote
2. ✅ If push confirmed: Proceed to next step
3. ✅ If push NOT confirmed:
   - **STOP workflow immediately**
   - **Verify git status**: Check if commits exist locally but not pushed
   - **IF unpushed commits exist**:
     - Spawn or resume the sub-agent
     - **Explicitly remind**: "You must git push immediately per Rule 04"
     - Wait for sub-agent to push
     - Verify push succeeded
   - **THEN continue workflow**

### Detection Methods

Main Agent can detect unpushed commits by:
```bash
# Check if local branch is ahead of remote
git status
# Output: "Your branch is ahead of 'origin/main' by N commits"

# Or check unpushed commits directly
git log origin/main..HEAD
# Output shows commits not yet pushed
```

### Enforcement Scenarios

**Scenario 1: Sub-agent reports completion without mentioning push**
```
Sub-agent: "Task completed. Files changed: [list]. Implementation done."

Main Agent MUST:
1. Check: "Did you git push?"
2. If not mentioned, verify git status
3. If unpushed commits detected, remind: "You must git push per Rule 04"
4. Wait for push confirmation
```

**Scenario 2: Sub-agent commits but doesn't push**
```
Sub-agent: "Changes committed successfully."

Main Agent MUST:
1. Immediately ask: "Did you git push?"
2. If no: "You must git push immediately per Rule 04"
3. If yes: "Confirm push with git status output"
4. Verify before proceeding
```

**Scenario 3: Sub-agent says "push failed"**
```
Sub-agent: "Commit succeeded but push failed due to [error]"

Main Agent MUST:
1. Review error (network issue? merge conflict?)
2. If recoverable error:
   - Guide sub-agent to resolve
   - Ensure push succeeds before proceeding
3. If unrecoverable:
   - Report to user
   - Note: Changes are safe locally
```

**Main Agent MUST NOT**:
- ❌ Accept completion reports without push confirmation
- ❌ Proceed to next task if commits are unpushed
- ❌ Assume sub-agent pushed without verification
- ❌ Skip push verification to save time

**Why This Matters**:
- Unpushed commits risk data loss
- Remote backup is critical for collaboration
- CI/CD pipelines need pushed commits
- Team visibility requires remote updates
- Rule 04 compliance depends on Main Agent enforcement

## Special Cases

### Merge Conflicts
If `git push` fails due to merge conflicts:
```bash
git push
# Error: Updates were rejected because remote contains work...

# Proper resolution:
git pull              # Pull remote changes
# Resolve conflicts if any
git add [files]       # Stage resolved files
git commit -m "..."   # Commit merge resolution (if needed)
git status            # Verify
git push              # Push again (automatic)
```

**Never use `--force` to override conflicts.**

### Branch Protection Rules
If remote has branch protection requiring reviews:
```bash
git push
# Error: Protected branch requires review...

# This is expected behavior
# Agent should report: "Changes committed and push attempted.
# Remote branch requires pull request review per repository settings."
```

**Do not attempt to bypass branch protection rules.**

### First Push to New Branch
```bash
git checkout -b new-feature-branch
# Make changes
git add [files]
git commit -m "..."
git status
git push -u origin new-feature-branch  # Use -u for first push to new branch
```

**The `-u` flag is allowed for setting upstream branch.**

### Network Issues
If `git push` fails due to network issues:
```bash
git push
# Error: Could not resolve host / Connection timeout

# Agent should:
# 1. Report the network error to user
# 2. Note that changes are committed locally and safe
# 3. Do not retry indefinitely
# 4. Do not use --force
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

### Why Automatic Push Matters
1. **Immediate Backup**: Changes are backed up to remote server instantly
2. **Collaboration**: Other developers/agents can see changes in real-time
3. **Continuous Integration**: CI/CD pipelines can process changes immediately
4. **Reduced Risk**: Minimizes risk of losing uncommitted work
5. **Transparency**: All work is immediately visible in remote repository
6. **Accountability**: Clear, immediate record of all changes

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

### Why Safety Restrictions
1. **Prevent Data Loss**: Forbidden operations could destroy commit history
2. **Protect Team**: Cannot overwrite other developers' work
3. **Maintain History**: Git history remains intact and recoverable
4. **Enable Recovery**: Any change can be reverted using standard git commands
5. **Build Trust**: Safety guarantees make automatic approval viable
6. **Industry Standards**: Follows git best practices for collaborative development

## Enforcement

### Mandatory Compliance

All agents **MUST**:
- Commit immediately after every change
- Push automatically after every commit
- Never ask for approval before git operations
- Only use allowed git operations
- Absolutely never use forbidden operations
- Follow the complete workflow for every change
- Treat automatic push as non-negotiable
- **ALWAYS verify code before committing** (Rule 05)
- **NEVER bypass verification** for any reason

### Violations

Any of the following constitutes a violation:

**Commit Violations**:
- Making multiple changes before committing
- Batching commits at the end of work
- Using vague or non-descriptive commit messages
- Omitting detailed explanations or bullet points
- Missing co-authorship attribution
- Failing to verify commit success
- Skipping commits for "small" changes
- **CRITICAL**: Committing code without verification (see Rule 05)
- **CRITICAL**: Missing verification status in commit message for code changes

**Push Violations**:
- Asking for approval before push
- Not pushing after successful commit
- Using any forbidden git operation
- Using `--force` flag in any command
- Batching multiple commits before pushing
- Skipping push after commit
- Any attempt to rewrite or destroy git history

**Main Agent Violations**:
- ❌ Accepting completion reports without verifying push
- ❌ Proceeding to next task when commits are unpushed
- ❌ Not checking git status to detect unpushed commits
- ❌ Failing to remind sub-agent to push when violation detected
- ❌ Skipping push verification to save time

### Critical Violations (Zero Tolerance)

The following violations are **CRITICAL** and trigger immediate corrective action:

1. ❌ **Committing code without verification**
   - Implementation agents committing directly
   - Main Agent committing before verification completes
   - Bypassing verification workflow (Rule 05)

2. ❌ **Committing code with failed verification**
   - Ignoring verification failures
   - Committing despite test failures
   - Proceeding with broken builds

3. ❌ **Missing verification status in code commit messages**
   - Not including verification results
   - Vague verification references
   - Incomplete verification reports

4. ❌ **Force push or destructive operations**
   - Force push: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**
   - Hard reset: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**
   - History rewriting: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**
   - Any operation with `--force`: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**

### Corrective Action

**For Commit Violations**:
1. **Stop immediately** and do not proceed with further changes
2. **Create proper commits** for any uncommitted changes
3. **Follow the correct format** for commit messages
4. **Verify each commit** before proceeding
5. **Report the violation** to maintain awareness

**For Critical Violations** (committing without verification):
1. **REVERT the commit immediately** using `git revert` or `git reset`
2. **Report to Main Agent** about the violation
3. **Run proper verification workflow** (Rule 05)
4. **Wait for ALL checks to PASS**
5. **Re-commit with verification status** once checks pass
6. **Document violation** in Learning Log

**For Forbidden Operations**:
1. **Stop immediately** if forbidden operation is about to be executed
2. **Do not proceed** with the forbidden operation
3. **Use safe alternative** if one exists
4. **Create a new commit** to fix issues instead
5. **Ask user for guidance** if truly stuck (rare cases only)

## Integration with Other Rules

### Rule 05 (Coding Practice and Agent Orchestration) - CRITICAL INTEGRATION
- **MANDATORY**: All code commits MUST go through verification workflow first
- Implementation agents report to Main Agent (never commit directly)
- Main Agent delegates to verification agent
- Main Agent commits ONLY after verification passes
- Commit messages MUST include verification status
- See Rule 05 for complete verification workflow

### Rule 03 (Dangerous Operations Safety)
- Git Safety Checkpoint required before dangerous operations
- ALL agents must commit and push before dangerous operations
- Dangerous operations blocked if git push fails

### Rule 07 (Specifications and Requirements)
- Commits apply to specification files (requirements.md, tasks.md)
- Specification updates are committed after verification
- Task completion commits include verification results

### Rule 08 (Language Conventions and Standards)
- Commits must follow language-specific standards
- Verification ensures standards compliance
- Learning Log updates are committed immediately

### Rule 02 (Directory Policy)
- Commits apply to files in all locations
- Stack files, rules, and specifications all follow this rule

### Rule 01 (Naming Conventions)
- Commits apply to rule files and all other files
- Naming convention changes are committed immediately

## Summary

**Core Workflow**:
```
Change → git add → git commit → git status → git push → verify → proceed
```

**For Code Changes**:
```
Implement → Report to Main → Verification (Rule 05) → ALL PASS →
git add → git commit (with verification) → git status → git push → verify → proceed
```

**Key Points**:
- ✅ Commit immediately after every change
- ✅ Push automatically after every commit
- ✅ Detailed commit messages with co-authorship
- ✅ Verification required for all code commits
- ✅ Main Agent verifies sub-agents pushed
- ✅ Only safe, non-destructive operations allowed
- ❌ Never batch commits
- ❌ Never ask for approval
- ❌ Never skip push
- ❌ Never use force push
- ❌ Never bypass verification for code

**This rule is non-negotiable.**

---
*Created: 2026-01-13*
*Merged from: Rule 04 (Work Commit Rules) and Rule 06 (Git Auto-Approval and Push)*
*Last Updated: 2026-01-13*
