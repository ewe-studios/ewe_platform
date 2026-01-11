# Git Auto-Approval and Push

## Purpose
This rule establishes automatic approval for git operations and mandatory automatic push after every successful commit, eliminating the need for user confirmation while maintaining strict safety guarantees.

## Rule

### Automatic Approval - No Confirmation Required
Agents **MUST NOT** ask for user approval before performing git operations. All git commits and pushes are **PRE-APPROVED** and should be executed automatically as part of the normal workflow.

### Automatic Push Requirement
After **EVERY** successful git commit and verification, agents **MUST**:

1. **Verify commit success** using `git status`
2. **Automatically push** to remote using `git push`
3. **Verify push success** before proceeding to next task
4. **No user confirmation** is required for any of these steps

### Complete Workflow
```
1. Make changes to file(s)
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
- **NO asking** for permission to commit
- **NO asking** for permission to push
- **NO manual approval** steps in the workflow
- **ALWAYS push** after successful commit + verification
- This applies to **ALL commits** without exception

### Safety Requirements - Destructive Operations FORBIDDEN

To ensure automatic approval is safe, the following operations are **ABSOLUTELY FORBIDDEN**:

#### ❌ FORBIDDEN Operations
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

#### ✅ ALLOWED Operations
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

This automatic approval system is safe because:

1. **Non-destructive operations only**: All allowed operations preserve history and data
2. **Standard push only**: Regular `git push` will fail if conflicts exist, forcing proper resolution
3. **No force operations**: Cannot overwrite remote history or other developers' work
4. **Rule 03 integration**: Commits follow strict message and verification requirements
5. **Atomic commits**: Each commit is verified before pushing
6. **Recoverable**: All operations can be undone using standard git recovery methods

## Examples

### Good Practice ✅

**Example 1: Create file, commit, and push automatically**
```bash
# After creating a new feature file
git add src/features/new-feature.js
git commit -m "$(cat <<'EOF'
Add new feature for user notifications

Implemented notification system to alert users of important events
in real-time using WebSocket connections.

Changes made:
- Created NotificationService class
- Added WebSocket connection handling
- Implemented notification queue system
- Added error handling and reconnection logic

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
```

**Example 2: Fix bug, commit, and push automatically**
```bash
# After fixing a bug
git add src/validators/email-validator.js tests/email-validator.test.js
git commit -m "$(cat <<'EOF'
Fix email validation for international domains

Fixed bug where email addresses with international TLDs
(e.g., .co.uk, .com.au) were incorrectly rejected.

Changes made:
- Updated email regex to support multi-part TLDs
- Added test cases for international domains
- Fixed validation error messages for clarity

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
```

**Example 3: Update documentation, commit, and push automatically**
```bash
# After updating documentation
git add README.md docs/api-guide.md
git commit -m "$(cat <<'EOF'
Update API documentation with authentication examples

Enhanced API documentation to include comprehensive authentication
examples and troubleshooting guidance.

Changes made:
- Added JWT authentication examples to API guide
- Updated README with authentication setup steps
- Added troubleshooting section for common auth errors
- Fixed formatting inconsistencies

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
```

**Example 4: Create rule file, commit, and push automatically**
```bash
# After creating a new rule file
git add .agents/rules/05-git-auto-approval-and-push.md
git commit -m "$(cat <<'EOF'
Add rule for automatic git approval and push

Established new rule requiring automatic push after every commit
without user approval, while maintaining strict safety guarantees.

Changes made:
- Created 05-git-auto-approval-and-push.md rule file
- Defined automatic approval requirements
- Listed forbidden destructive operations
- Provided complete workflow examples
- Documented safety guarantees

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git status  # Verify commit succeeded
git push    # Automatic push - no approval needed
```

### Bad Practice ❌

**Example 1: Asking for approval**
```bash
git add src/feature.js
git commit -m "Add feature"
# Agent asks: "Should I push this to remote?"

❌ Never ask for approval to push
❌ Push should be automatic after commit verification
```

**Example 2: Not pushing automatically**
```bash
git add src/feature.js
git commit -m "Add feature"
git status
# Agent stops here without pushing

❌ Must automatically push after commit verification
❌ Workflow is incomplete without push
```

**Example 3: Using force push**
```bash
git add src/feature.js
git commit -m "Add feature"
git push --force

❌ Force push is absolutely forbidden
❌ Destructive operation violates safety requirements
```

**Example 4: Batching multiple commits before pushing**
```bash
# Agent makes commit 1
git commit -m "Change A"
# Agent makes commit 2
git commit -m "Change B"
# Agent makes commit 3
git commit -m "Change C"
# Then pushes all at once
git push

❌ Must push after each individual commit
❌ Should push after commit 1, then commit 2, then commit 3
```

**Example 5: Using hard reset**
```bash
# Something went wrong
git reset --hard HEAD~1
git push

❌ Hard reset is forbidden - destroys commit history
❌ Cannot use destructive operations even to "fix" issues
```

**Example 6: Amending and force pushing**
```bash
git commit -m "Initial commit"
git push
# Agent realizes something was wrong
git commit --amend
git push --force

❌ Force push is forbidden
❌ Should create a new commit instead of amending
```

## Workflow Integration

### Integration with Rule 03 (Work Commit Rules)

Rule 03 establishes the commit workflow:
```
Change → git add → git commit → git status (verify)
```

Rule 05 (this rule) extends it:
```
Change → git add → git commit → git status (verify) → git push (auto)
```

Combined workflow:
1. **Make change** to file(s)
2. **Stage changes** with `git add`
3. **Create commit** with detailed message and co-authorship (Rule 03)
4. **Verify commit** with `git status` (Rule 03)
5. **Push automatically** with `git push` (Rule 05 - this rule)
6. **Verify push** succeeded
7. **Proceed** to next task

### Multiple Changes Workflow
```
Change 1 → add → commit → verify → push → verify →
Change 2 → add → commit → verify → push → verify →
Change 3 → add → commit → verify → push → verify →
... and so on
```

**Each commit is pushed individually before moving to the next change.**

## Rationale

### Why Automatic Approval
1. **Efficiency**: Eliminates unnecessary approval steps in the workflow
2. **Consistency**: Ensures every commit is pushed without human error
3. **Real-time Backup**: Every change is immediately backed up to remote
4. **Team Visibility**: Changes are immediately visible to all team members
5. **Reduced Friction**: Streamlines development workflow
6. **Trust**: Assumes agents follow rules and produce quality commits

### Why Automatic Push
1. **Immediate Backup**: Changes are backed up to remote server instantly
2. **Collaboration**: Other developers/agents can see changes in real-time
3. **Continuous Integration**: CI/CD pipelines can process changes immediately
4. **Reduced Risk**: Minimizes risk of losing uncommitted work
5. **Transparency**: All work is immediately visible in remote repository
6. **Accountability**: Clear, immediate record of all changes

### Why Safety Restrictions
1. **Prevent Data Loss**: Forbidden operations could destroy commit history
2. **Protect Team**: Cannot overwrite other developers' work
3. **Maintain History**: Git history remains intact and recoverable
4. **Enable Recovery**: Any change can be reverted using standard git commands
5. **Build Trust**: Safety guarantees make automatic approval viable
6. **Industry Standards**: Follows git best practices for collaborative development

### Why No Approval Needed
1. **Rule 03 Protection**: Commits are already verified and well-formed
2. **Non-destructive**: Only safe operations are allowed
3. **Recoverable**: All changes can be undone if needed
4. **Atomic**: Each commit is small and focused
5. **Transparent**: Full commit history with detailed messages
6. **Safe by Design**: System cannot perform destructive operations

## Enforcement

### Mandatory Compliance
All agents **MUST**:
- Push automatically after every commit verification
- Never ask for approval before git operations
- Only use allowed git operations
- Absolutely never use forbidden operations
- Follow the complete workflow for every change
- Treat automatic push as non-negotiable

### Violations

Any of the following constitutes a serious violation:
- Asking for approval before commit or push
- Not pushing after successful commit
- Using any forbidden git operation
- Using `--force` flag in any command
- Batching multiple commits before pushing
- Skipping push after commit verification
- Any attempt to rewrite or destroy git history

### Corrective Action

When a violation is detected:
1. **Stop immediately** if forbidden operation is about to be executed
2. **Do not proceed** with the forbidden operation
3. **Use safe alternative** if one exists
4. **Report violation** to maintain awareness
5. **Continue with safe workflow** following all rules

### Zero Tolerance for Destructive Operations

The following have **ZERO TOLERANCE** enforcement:
- Force push: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**
- Hard reset: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**
- History rewriting: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**
- Any operation with `--force`: **NEVER ALLOWED UNDER ANY CIRCUMSTANCES**

If a situation arises where these seem necessary:
1. **Do not perform the operation**
2. **Create a new commit** to fix the issue instead
3. **Follow proper git recovery procedures** (revert, new commits)
4. **Ask user for guidance** if truly stuck (rare cases only)

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

## Cross-Rule Integration

### Works With
- **Rule 01**: File naming applies to git-tracked files
- **Rule 02**: Directory policy applies to git repositories
- **Rule 03**: Commit requirements are prerequisite for this rule
- **Rule 04**: Agent orchestration applies to all git operations

### Dependencies
This rule **DEPENDS ON** Rule 03 (Work Commit Rules):
- Rule 03 ensures commits are well-formed and verified
- Rule 05 adds automatic push after Rule 03's verification step
- Both rules together create complete change-to-remote workflow

### Rule Priority
When rules conflict (they shouldn't):
1. **Safety first**: Forbidden operations remain forbidden
2. **Rule 03 + 05**: Combined workflow takes precedence
3. **No force**: Safety restrictions override all other rules

## Summary

**Core Principle**: Trust the agent to commit and push automatically, but absolutely forbid any operation that could destroy data or corrupt history.

**Key Points**:
- ✅ Automatic approval - no confirmation needed
- ✅ Automatic push after every commit
- ✅ Only safe, non-destructive operations allowed
- ❌ Never ask for approval
- ❌ Never use force push
- ❌ Never use hard reset
- ❌ Never rewrite history

---
*Created: 2026-01-11*
