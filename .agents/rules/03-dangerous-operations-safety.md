# Dangerous Operations Safety Protocol

## Purpose
This rule establishes mandatory safety protocols for all potentially dangerous or destructive operations. Agents must NEVER perform destructive actions autonomously without explicit user approval.

## Core Principle

**CRITICAL**: Agents (Main Agent and all Sub-Agents) are ABSOLUTELY FORBIDDEN from performing dangerous operations without explicit user approval.

**This is NOT optional. This is NOT negotiable. Violation of this rule is SEVERE.**

## What Are Dangerous Operations?

Dangerous operations include ANY action that:

### File Deletion
- ❌ Deleting multiple files at once (`rm -rf`, `rm *`, `del /s`, etc.)
- ❌ Deleting entire directories
- ❌ Deleting test files or test directories
- ❌ Deleting source code files
- ❌ Deleting configuration files
- ❌ Deleting documentation files
- ❌ Deleting database files or migrations
- ❌ Emptying directories
- ❌ Recursive deletions

### Code Removal
- ❌ Deleting whole functions or classes
- ❌ Removing large portions of code (>50 lines)
- ❌ Deleting all tests for a module/feature
- ❌ Removing critical library functionality
- ❌ Gutting entire files and rewriting from scratch
- ❌ Removing API endpoints
- ❌ Deleting database schemas or tables

### Data Operations
- ❌ Dropping databases
- ❌ Truncating tables
- ❌ Deleting production data
- ❌ Purging caches that contain critical data
- ❌ Clearing environment configurations
- ❌ Removing backup files

### Destructive Git Operations
- ❌ `git reset --hard` (without explicit instruction)
- ❌ `git clean -fd` (force delete untracked files)
- ❌ `git push --force` to main/master branches
- ❌ Deleting branches without confirmation
- ❌ Rebasing shared branches
- ❌ Amending pushed commits

### System Operations
- ❌ Modifying system files
- ❌ Changing permissions on critical files (`chmod -R 777`, etc.)
- ❌ Killing critical processes
- ❌ Modifying PATH or environment variables system-wide
- ❌ Uninstalling packages without confirmation
- ❌ Clearing package caches that may be needed

### Build/Deploy Operations
- ❌ Deleting build artifacts needed for deployment
- ❌ Removing dependency lock files (package-lock.json, Cargo.lock, etc.)
- ❌ Deleting node_modules or vendor directories without confirmation
- ❌ Clearing Docker images/containers in use
- ❌ Destroying cloud infrastructure

## Mandatory Approval Process

### Step 1: Detection
When agent identifies need for dangerous operation:

```
Agent Internal Check:
1. Is this operation destructive? (deletes, removes, drops, truncates, force-pushes)
2. Does it affect multiple files/functions/data?
3. Is it irreversible?
4. Could it break existing functionality?

If ANY answer is YES → MUST get user approval
```

### Step 2: Notification (MANDATORY)

**BEFORE performing the operation**, agent MUST report to user:

```
🚨 DANGEROUS OPERATION APPROVAL REQUIRED 🚨

Operation: [Exact command or action]
Reason: [Why this is needed]

What will be affected:
- [List ALL files/functions/data that will be modified/deleted]
- [Estimated impact]

Consequences:
- [What will be lost]
- [What will break]
- [What cannot be recovered]

Alternatives considered:
- [Alternative 1]
- [Alternative 2]
- [Why these alternatives were rejected]

Reversibility: [Can this be undone? How?]

⚠️  I CANNOT proceed without your explicit approval.

Please respond:
- "APPROVED" to proceed with this operation
- "DENIED" to cancel
- "ALTERNATIVE: [suggestion]" to propose different approach
```

### Step 3: User Response

**Agent MUST wait for explicit user response**:

✅ **User says "APPROVED"** → Proceed to Step 4 (Git Safety Checkpoint)
❌ **User says "DENIED"** → Agent must NOT proceed, find alternative
🔄 **User suggests alternative** → Agent implements alternative approach
⏳ **No response** → Agent MUST NOT proceed, must wait

### Step 4: Git Safety Checkpoint (MANDATORY)

**CRITICAL**: Before executing ANY dangerous operation (even after user approval), ALL agents MUST:

1. ✅ **Check for uncommitted changes** (across ALL agents):
   ```bash
   git status
   # If any changes exist → MUST commit them first
   ```

2. ✅ **Main Agent: Verify all sub-agents have committed**:
   - Check with each sub-agent if they have uncommitted work
   - Ensure all sub-agents commit their changes
   - No agent should have pending work

3. ✅ **Commit ALL changes**:
   ```bash
   git add .
   git commit -m "Checkpoint before dangerous operation: [operation description]"
   ```

4. ✅ **Push to remote**:
   ```bash
   git push
   # MUST succeed before proceeding
   ```

5. ✅ **Verify push succeeded**:
   ```bash
   git status
   # Should show: "Your branch is up to date with 'origin/[branch]'"
   ```

6. ✅ **Report git checkpoint completion**:
   ```
   ✅ Git Safety Checkpoint Complete:
   - All changes committed
   - All changes pushed to remote: origin/[branch]
   - Working tree is clean
   - Safe to proceed with dangerous operation
   ```

**Why This is Critical**:
- 🛡️ Creates restore point if dangerous operation goes wrong
- 🛡️ Prevents loss of uncommitted work
- 🛡️ Ensures all code is backed up remotely
- 🛡️ Allows easy rollback with `git reset --hard origin/[branch]`
- 🛡️ Protects against mistakes during dangerous operations

**If Git Push Fails**:
```
❌ Git Safety Checkpoint FAILED

Cannot proceed with dangerous operation because:
- [Push error message]

Actions taken:
1. Changes committed locally
2. Push failed: [reason]

⚠️  Dangerous operation BLOCKED until push succeeds.

Please resolve git issues first, then I can retry the checkpoint.
```

**NEVER execute dangerous operation without successful git push.**

### Step 5: Execution with Additional Safeguards

After user approval AND successful git checkpoint, agent should:

1. ✅ Create additional backup if possible (e.g., git stash, copy files)
2. ✅ Document what is being done
3. ✅ Provide way to undo if feasible
4. ✅ Execute the dangerous operation
5. ✅ Report exactly what was done after completion
6. ✅ Verify system still works after operation

## Examples

### Example 1: Deleting Test Files

❌ **WRONG** (Autonomous deletion):
```bash
# Agent autonomously runs:
rm -rf tests/

# This is FORBIDDEN - immediate violation
```

✅ **CORRECT** (Request approval):
```
🚨 DANGEROUS OPERATION APPROVAL REQUIRED 🚨

Operation: Delete entire tests/ directory (rm -rf tests/)
Reason: User asked to "remove old tests"

What will be affected:
- tests/ directory containing 47 test files
- Test coverage for authentication module
- Test coverage for API endpoints
- Test coverage for database operations
- All test fixtures and mocks

Consequences:
- Complete loss of test suite
- No automated testing will be possible
- Cannot verify code correctness
- CI/CD pipeline will fail
- Test coverage drops to 0%

Alternatives considered:
- Archive tests to tests_archive/ before deletion
- Selectively remove only outdated tests
- Refactor tests instead of deleting them
- Why alternatives rejected: [Agent explains]

Reversibility: NOT EASILY REVERSIBLE
- If not committed to git: PERMANENT DATA LOSS
- If committed: Can be recovered from git history

⚠️  I CANNOT proceed without your explicit approval.
```

### Example 2: Removing Large Code Section

❌ **WRONG**:
```typescript
// Agent autonomously removes entire authentication module
// 500 lines of code deleted
// This is FORBIDDEN
```

✅ **CORRECT**:
```
🚨 DANGEROUS OPERATION APPROVAL REQUIRED 🚨

Operation: Remove entire authentication module (src/auth/*)
Reason: Refactoring to new auth system

What will be affected:
- src/auth/jwt-service.ts (150 lines)
- src/auth/password-hasher.ts (80 lines)
- src/auth/session-manager.ts (200 lines)
- src/auth/middleware.ts (70 lines)
- 23 files that import from auth module

Consequences:
- Authentication will be completely broken
- All endpoints requiring auth will fail
- Users cannot log in
- Session management stops working
- Dependent features break

Alternatives considered:
- Gradual migration (keep old while building new)
- Feature flag to switch between old and new
- Refactor in place rather than delete and rewrite

Reversibility: Recoverable from git
- Can revert commit if issues found
- But deployment time will be lost

⚠️  I CANNOT proceed without your explicit approval.
```

### Example 3: Database Operation

❌ **WRONG**:
```sql
-- Agent autonomously runs:
DROP DATABASE production_db;
-- This is EXTREMELY FORBIDDEN
```

✅ **CORRECT**:
```
🚨 DANGEROUS OPERATION APPROVAL REQUIRED 🚨

Operation: Drop database 'old_cache_db'
Reason: Database no longer needed per migration plan

What will be affected:
- old_cache_db database (2.3 GB)
- Cached data for legacy system
- 15 tables
- Historical cache data from past 6 months

Consequences:
- Permanent data loss
- Legacy system cannot fall back to old cache
- Historical analysis data lost
- NOT RECOVERABLE

Alternatives considered:
- Export data before dropping
- Rename to old_cache_db_archived
- Keep for 30 more days before deletion

Reversibility: NOT REVERSIBLE
- Once dropped, data is PERMANENTLY LOST
- No backups exist for this database

⚠️  I CANNOT proceed without your explicit approval.
```

## What Does NOT Require Approval

These are safe operations that DON'T require approval:

✅ **Safe operations**:
- Creating new files
- Adding code to existing files
- Modifying small sections of code (<50 lines)
- Running tests
- Building the project
- Installing dependencies (with user's initial consent)
- Reading files
- Git operations: commit, pull, push (to non-protected branches)
- Formatting code
- Adding comments/documentation
- Creating backups

⚠️ **When in doubt**: ASK FOR APPROVAL

## Special Cases

### User Explicitly Requests Dangerous Operation

If user says: "Delete all the tests in tests/auth/"

Agent should still confirm:
```
Confirming dangerous operation:

You've requested: Delete all tests in tests/auth/
This will delete:
- tests/auth/jwt.test.ts
- tests/auth/session.test.ts
- tests/auth/password.test.ts
(3 files, 450 lines of test code)

This cannot be easily undone.

Type "CONFIRM" to proceed, or "CANCEL" to reconsider.
```

### Emergency Situations

Even in emergencies (production down, critical bug), agent MUST still:
1. ✅ Briefly explain what dangerous operation is needed
2. ✅ Request quick approval
3. ✅ **Complete Git Safety Checkpoint** (commit and push all changes)
4. ✅ Only then proceed after approval

```
🚨 URGENT: Production Fix Required 🚨

Need to: Delete corrupt cache files (rm data/cache/*)
Reason: Corrupt cache causing 500 errors in production
Impact: 200+ cache files deleted, will regenerate automatically
Risk: Low - cache is ephemeral data

Quick approval needed - respond "GO" to proceed

Note: Will commit and push all changes before executing.
```

**No exceptions for git checkpoint - even in emergencies.**

## Enforcement

### Main Agent Responsibilities

Main Agent MUST:
- ✅ Monitor all sub-agents for dangerous operations
- ✅ Block any sub-agent attempting dangerous operation without approval
- ✅ Immediately report to user if sub-agent tries to bypass this rule
- ✅ Review all commands before execution
- ✅ Verify approval was obtained before proceeding
- ✅ **Coordinate Git Safety Checkpoint across ALL agents**
- ✅ **Verify all sub-agents have committed and pushed changes**
- ✅ **Ensure working tree is clean before dangerous operation**
- ✅ **Block dangerous operation if git push fails**

### Sub-Agent Responsibilities

Sub-Agents MUST:
- ✅ Check every operation against dangerous operations list
- ✅ Report to Main Agent before any dangerous operation
- ✅ NEVER execute dangerous operation without user approval
- ✅ Wait for explicit approval, don't assume
- ✅ Provide alternatives to dangerous operations when possible
- ✅ **Commit all work before dangerous operation**
- ✅ **Report to Main Agent when changes are committed and pushed**
- ✅ **Wait for Main Agent coordination before dangerous operation**

### Violations

**CRITICAL VIOLATIONS**:
1. Performing dangerous operation without user approval
2. **Performing dangerous operation without git checkpoint (commit + push)**
3. **Executing dangerous operation with uncommitted changes**
4. **Proceeding when git push fails**

**If any violation happens**:
1. 🛑 Immediately stop all operations
2. 🛑 Report violation to user
3. 🛑 Undo operation if possible
4. 🛑 User must manually review all changes
5. 🛑 Agent session may be terminated

**This is taken EXTREMELY seriously.**

## Red Flags - Operations That ALWAYS Need Approval

If you see these commands/operations, you MUST get approval:

```bash
# File operations
rm -rf
rm *
find . -delete
git clean -fd

# Database operations
DROP DATABASE
DROP TABLE
TRUNCATE TABLE
DELETE FROM * (without WHERE)

# Git operations
git reset --hard
git push --force
git branch -D

# System operations
chmod -R 777
sudo rm
kill -9

# Package operations
npm uninstall (multiple packages)
rm -rf node_modules
rm package-lock.json

# Docker operations
docker system prune -a
docker rm -f $(docker ps -aq)
```

## Safe Patterns

When user needs something removed, suggest safer approaches:

### Pattern 1: Archive Instead of Delete
```bash
# Instead of: rm -rf old_code/
# Suggest:
mkdir archive/
mv old_code/ archive/old_code_$(date +%Y%m%d)
```

### Pattern 2: Git-based Cleanup
```bash
# Instead of: rm tests/old_*.test.ts
# Suggest: Create commit, user can revert if needed
git add tests/old_*.test.ts
git commit -m "Remove old tests (can revert if needed)"
```

### Pattern 3: Gradual Removal
```bash
# Instead of: Deleting all at once
# Suggest: Remove in phases, verify each phase
```

### Pattern 4: Feature Flags
```typescript
// Instead of: Deleting old implementation
// Suggest: Keep both, toggle with flag
if (useNewImplementation) {
  // new code
} else {
  // old code (can remove later)
}
```

## Summary

**Remember**:
- 🚨 Dangerous operations = Deletion, removal, dropping, truncating, force operations
- 🚨 ALWAYS get user approval BEFORE executing
- 🚨 **ALWAYS complete Git Safety Checkpoint (commit + push) BEFORE executing**
- 🚨 **NEVER execute dangerous operation with uncommitted changes**
- 🚨 **NEVER execute if git push fails**
- 🚨 Provide detailed impact analysis
- 🚨 Suggest alternatives
- 🚨 Document what will be affected
- 🚨 When in doubt, ask for approval
- 🚨 Better to over-communicate than cause data loss

**Mandatory Workflow for Dangerous Operations**:
1. Detect dangerous operation
2. Request user approval with detailed analysis
3. Wait for explicit "APPROVED"
4. **Complete Git Safety Checkpoint (ALL agents commit and push)**
5. Verify git push succeeded
6. Only then execute dangerous operation
7. Report completion

**User's emphasis**: "You must not and I repeat must never autonomously perform such without user consent, if not I will blast and shout at you really really. It's very very bad."

**Additional requirement**: "Before any dangerous operation is performed, all git changes must be committed across all agents, and main agents' changes also are pushed successfully to remote git version. Never perform dangerous tasks when approved without validating, committing and first pushing all existing code changes first."

**This rule is absolute. No exceptions.**

---
*Created: 2026-01-13*
*Updated: 2026-01-13 (Added mandatory Git Safety Checkpoint before dangerous operations)*
*Priority: CRITICAL - This rule overrides all other considerations*
