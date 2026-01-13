# Coding Practice: Agent Orchestration and Mandatory Verification

## Purpose
This rule establishes the mandatory practice for code development, requiring specialized agent orchestration with **MANDATORY CODE VERIFICATION** before any commit. This is an **IRON-CLAD REQUIREMENT** with **ZERO TOLERANCE** for violations.

## Core Principles

### 1. Main Agent as Orchestrator
The Main Agent **MUST**:
- Act as controller and orchestrator ONLY
- NEVER perform coding tasks directly
- Launch specialized agents for all code work
- **MANDATORY**: Delegate to verification agents after implementation
- Coordinate specification updates
- Commit code ONLY after verification passes

### 2. Agent Identity and Verification Authority
**CRITICAL DISTINCTION**: Only the Main Agent can spawn verification agents.

**Main Agent**:
- ✅ The agent directly interacting with the user
- ✅ The orchestrator at the top of the agent hierarchy
- ✅ **ONLY agent with authority to spawn verification agents**
- ✅ Spawns implementation agents, specification agents, and verification agents

**Sub-Agents** (Implementation, Specification, etc.):
- ❌ **NEVER spawn verification agents**
- ❌ **NOT the Main Agent** - they are delegated workers
- ✅ Report completion to Main Agent
- ✅ Wait for Main Agent to coordinate verification

**Agent Identity Rule**:
```
If you were spawned by another agent → You are a SUB-AGENT
If you are directly interacting with user → You are the MAIN AGENT

SUB-AGENTS: Report to Main Agent, NEVER spawn verification agents
MAIN AGENT: Spawn verification agents, orchestrate all workflows
```

### 3. Verification-First Workflow
**CRITICAL REQUIREMENT**: NO code is EVER committed without verification.

```
Implementation → Report to Main → Verification → Update Spec → Commit
```

## Mandatory Workflow

### Phase 1: Implementation

#### Main Agent Responsibilities
1. **Breaks down work** into specific tasks
2. **Identifies specifications** (references `.agents/specifications/NN-spec-name/`)
3. **Launches implementation agents** (up to 10 concurrent)
4. **WAITS for completion reports** from all agents
5. **DOES NOT COMMIT** anything yet

#### Implementation Agent Requirements
Each implementation agent **MUST**:

**Agent Identity Awareness**:
1. ✅ **Know you are a SUB-AGENT** (spawned by Main Agent)
2. ✅ **Understand you do NOT have verification authority**
3. ✅ **NEVER spawn verification agents** (only Main Agent can)

**Before Starting Work**:
1. ✅ Read `AGENTS.md` file
2. ✅ Load all rules from `.agents/rules/*`
3. ✅ Read specification `requirements.md` and `tasks.md`
4. ✅ Read relevant language stack files from `.agents/stacks/[language].md`
5. ✅ Understand what to build and standards to follow

**During Work**:

**MANDATORY: Test-Driven Development (TDD) Workflow**

Implementation agents **MUST** follow TDD whenever possible:

1. ✅ **Write the test FIRST** (before implementation code)
   - Write test with WHY/WHAT documentation
   - Test describes the expected behavior
   - Test should be specific to one requirement/behavior

2. ✅ **Verify test FAILS** for the right reason
   - Run the test to confirm it fails
   - Ensure failure message indicates missing functionality (not syntax errors)
   - If test passes before implementation, the test is wrong or feature already exists

3. ✅ **Implement minimum code** to make test pass
   - Write simplest code that satisfies the test
   - Follow stack standards and code simplicity principles
   - Don't over-engineer or add functionality not tested

4. ✅ **Verify test PASSES**
   - Run the test to confirm it now passes
   - Ensure implementation actually fixed the failure

5. ✅ **Refactor if needed** (while keeping test green)
   - Simplify code if possible
   - Apply DRY where it improves clarity
   - Ensure test still passes after refactoring

6. ✅ **Repeat cycle** for next test/requirement
   - Continue until all task requirements are implemented
   - Each cycle: Write test → Red → Implement → Green → Refactor

**TDD Benefits**:
- Tests prove code works before you write it
- Tests document requirements as executable specifications
- Prevents over-engineering (only write what's tested)
- Catches regressions immediately
- Makes refactoring safer (tests catch breakage)

**When TDD May Not Apply**:
- Exploratory/spike work where requirements are unclear
- Refactoring existing code with good test coverage
- Fixing build/infrastructure issues
- In these cases: Write tests DURING implementation, not after

**TDD Enforcement**:
- ✅ Follow TDD cycle whenever implementing new functionality
- ✅ Document test-first approach in completion report
- ❌ **USER WILL SHOUT AT YOU** for writing implementation before tests
- ❌ **USER WILL SHOUT AT YOU** for not verifying tests fail first

**After TDD Cycles Complete**:
1. ✅ Follow all conventions and best practices
2. ✅ Keep track of what was changed

**After Completing Work**:
1. ✅ **SELF-REVIEW implementation quality** (see Critical Self-Review section below)
2. ✅ **Document learnings** (see Learning Documentation section below)
3. ✅ **REPORT to Main Agent** (NEVER commit directly)
4. ✅ Provide list of changed files
5. ✅ Describe what was implemented
6. ✅ Note which language(s) were used
7. ✅ Reference specification if applicable
8. ✅ **STOP and WAIT** for Main Agent

**Implementation Agent MUST NOT**:
- ❌ Commit code directly
- ❌ Push to remote
- ❌ Update tasks.md directly
- ❌ Skip reporting to Main Agent
- ❌ Proceed without Main Agent approval
- ❌ **Spawn verification agents** (ONLY Main Agent has this authority)

#### Test Documentation Requirements (MANDATORY)

**CRITICAL**: Every test MUST include documentation explaining why it exists.

**Why This Matters**:
- Future agents need to understand test purpose and importance
- Prevents accidental deletion of critical tests
- Makes debugging test failures faster
- Documents edge cases and business rules directly where they're tested
- **Avoids polluting learnings.md with task-specific test details**

**Test Documentation Format** (Language-Agnostic):

All tests MUST include a comment block explaining:
1. **Why**: Why this test exists (what problem/bug/requirement does it validate)
2. **What**: What specific behavior is being tested
3. **Importance**: Why this test matters (optional but recommended for critical tests)

**Format Examples by Stack**:

**Rust**:
```rust
/// WHY: Validates token expiration at exactly midnight (edge case from bug #234)
/// WHAT: Token with midnight expiry should be treated as expired
/// IMPORTANCE: Without this, users could access system for extra day after expiry
#[test]
fn test_token_expiry_at_midnight() {
    let token = create_token_with_expiry("2024-01-15T00:00:00Z");
    assert!(is_expired(&token));
}

/// WHY: Ensures connection pool doesn't deadlock under high load (production incident 2024-01-10)
/// WHAT: Spawning 100 concurrent requests should not exhaust pool or deadlock
#[tokio::test]
async fn test_connection_pool_under_load() {
    let handles: Vec<_> = (0..100)
        .map(|_| tokio::spawn(async { db::query("SELECT 1").await }))
        .collect();

    for handle in handles {
        assert!(handle.await.is_ok());
    }
}
```

**TypeScript/JavaScript**:
```typescript
/**
 * WHY: User profile images must be resized before upload (requirement from PM)
 * WHAT: Uploading 4K image should automatically resize to 512x512
 * IMPORTANCE: Prevents S3 cost explosion (4K images are 10x larger)
 */
test('should resize large images before upload', async () => {
  const largeImage = createMockImage(3840, 2160);
  const result = await uploadUserProfile(largeImage);

  expect(result.dimensions).toEqual({ width: 512, height: 512 });
});

/**
 * WHY: Rate limiter must track per-IP, not per-user (security requirement)
 * WHAT: Same IP with different users should hit rate limit
 * IMPORTANCE: Prevents distributed brute-force attacks
 */
test('rate limiter tracks by IP address', async () => {
  const ip = '192.168.1.1';

  for (let i = 0; i < 100; i++) {
    await makeRequest({ ip, user: `user_${i}` });
  }

  await expect(makeRequest({ ip, user: 'another_user' }))
    .rejects.toThrow('Rate limit exceeded');
});
```

**Python**:
```python
def test_token_expiry_at_midnight():
    """
    WHY: Validates token expiration at exactly midnight (edge case from bug #234)
    WHAT: Token with midnight expiry should be treated as expired
    IMPORTANCE: Without this, users could access system for extra day after expiry
    """
    token = create_token_with_expiry("2024-01-15T00:00:00Z")
    assert is_expired(token)

def test_webhook_fires_after_db_commit():
    """
    WHY: Webhook must fire AFTER db commit, not before (data consistency requirement)
    WHAT: If db commit fails, webhook should not be sent
    IMPORTANCE: Prevents webhook notifications for data that doesn't exist in DB
    """
    with mock.patch('db.commit', side_effect=DBError):
        with pytest.raises(DBError):
            process_payment(payment_data)

    # Webhook should NOT have been called
    assert mock_webhook.call_count == 0
```

**Documentation Guidelines**:

✅ **DO**:
- Write concise comments (2-4 lines for WHY/WHAT, 1 line for IMPORTANCE)
- Reference bug numbers, tickets, or production incidents when relevant
- Explain business rules and edge cases
- Document non-obvious test requirements
- Use plain language (avoid jargon)

❌ **DON'T**:
- Write obvious comments ("tests that addition works")
- Repeat what the code already says
- Write essays (keep it brief and scannable)
- Document in learnings.md what should be in test comments
- Omit the "WHY" (this is the most important part!)

**What Goes in Test Comments vs learnings.md**:

**Test Comments** (task-specific, narrow focus):
- Why THIS specific test exists
- What THIS test validates
- Edge cases for THIS feature
- References to specific bugs/tickets
- Business rules for THIS functionality

**learnings.md** (bigger picture, broader insights):
- Patterns that work across multiple tests
- Common testing pitfalls for this specification
- Testing strategies that proved effective
- Non-obvious testing insights that apply broadly

**Example Decision Tree**:
```
"We discovered token expiry at midnight causes issues"
├─ Test comment: "WHY: Validates token expiration at exactly midnight (bug #234)"
└─ learnings.md: "Must test time-boundary edge cases (midnight, year rollover, DST changes)"

"Webhook must fire after DB commit"
├─ Test comment: "WHY: Webhook fires AFTER db commit (data consistency requirement)"
└─ learnings.md: "Always test failure paths to ensure no side effects when operations fail"
```

**Enforcement**:

Implementation agents MUST:
- ✅ Add WHY/WHAT comments to every test
- ✅ Keep comments concise (2-5 lines total)
- ✅ Focus learnings.md on broader patterns, not individual test details
- ❌ **USER WILL SHOUT AT YOU** for tests without documentation
- ❌ **USER WILL SHOUT AT YOU** for polluting learnings.md with task-specific test details

#### Critical Self-Review Before Reporting Completion

**MANDATORY**: Before reporting completion to Main Agent, implementation agents **MUST** perform a thorough self-review to ensure quality and completeness.

**Self-Review Checklist** (ALL items MUST pass):

1. ✅ **Completeness Check**:
   - Implementation fully satisfies the task requirements
   - All requirements from requirements.md are met
   - No partial or incomplete implementations
   - No placeholder/fake code that looks complete but isn't functional

2. ✅ **Code Quality Check**:
   - Logic is clear, coherent, and not confusing
   - Code follows stack conventions (from `.agents/stacks/[language].md`)
   - No misleading variable names or functions
   - Proper error handling implemented
   - Edge cases considered and handled

3. ✅ **Code Simplicity and Clarity Check** (CRITICAL):
   - **Ask yourself: Can this code be simplified further?**
   - Break down overly nested functions (max 2-3 levels of nesting)
   - Extract complex logic into well-named helper functions
   - Function size: Keep functions small and focused (prefer 20-30 lines max)
   - **DRY (Don't Repeat Yourself) vs Clarity Trade-off**:
     * ✅ Use DRY when it improves clarity and reduces complexity
     * ✅ It's OK to duplicate small logic (2-5 lines) if abstraction adds complexity
     * ✅ Prefer inline clarity over forced abstraction
     * ❌ Don't create convoluted abstractions just to avoid 3 lines of duplication
   - **Clarity Principles**:
     * Code should read like prose: clear intent, obvious flow
     * Prefer explicit over clever: straightforward > "smart tricks"
     * If you need comments to explain logic, consider simplifying the code first
     * Names should eliminate the need for comments
   - **THINK HARD about simplification or USER WILL SHOUT AT YOU**

4. ✅ **Requirements Alignment Check**:
   - Review tasks.md - ensure task being reported is actually complete
   - Review requirements.md - ensure all related requirements are satisfied
   - Verify implementation matches the specification's intent
   - No deviation from specified behavior without justification

5. ✅ **Test Coverage Check**:
   - Tests exist for new functionality
   - Tests cover happy paths and edge cases
   - Tests are meaningful (not fake tests that always pass)
   - Test names clearly describe what they validate
   - **Every test has WHY/WHAT documentation** (see Test Documentation Requirements above)
   - Test documentation is concise (2-5 lines, not essays)

**If ANY Self-Review Check Fails**:
- ❌ DO NOT report completion to Main Agent
- ✅ Fix the issues identified
- ✅ Re-run the self-review checklist
- ✅ Only report completion when ALL checks pass

**Code Simplicity Examples**:

❌ **BAD - Overly nested, hard to follow**:
```rust
fn process_user(user: User) -> Result<Response> {
    if user.is_active {
        if let Some(profile) = user.profile {
            if profile.is_complete() {
                if let Ok(data) = fetch_data(&profile) {
                    if validate(&data) {
                        return Ok(Response::new(data));
                    }
                }
            }
        }
    }
    Err(Error::Invalid)
}
```

✅ **GOOD - Flattened, clear early returns**:
```rust
fn process_user(user: User) -> Result<Response> {
    if !user.is_active { return Err(Error::Inactive); }

    let profile = user.profile.ok_or(Error::NoProfile)?;
    if !profile.is_complete() { return Err(Error::Incomplete); }

    let data = fetch_data(&profile)?;
    validate(&data)?;

    Ok(Response::new(data))
}
```

❌ **BAD - Over-abstracted DRY (harder to understand)**:
```rust
fn create_user_handler(req: Request) -> Result<Response> {
    process_entity(req, UserValidator, UserCreator, user_response_mapper)
}

fn update_user_handler(req: Request) -> Result<Response> {
    process_entity(req, UserValidator, UserUpdater, user_response_mapper)
}

// Now you need to read process_entity implementation to understand flow
```

✅ **GOOD - Some duplication, but crystal clear**:
```rust
fn create_user_handler(req: Request) -> Result<Response> {
    let input = req.json::<UserInput>()?;
    validate_user_input(&input)?;
    let user = db::create_user(input).await?;
    Ok(Response::json(user))
}

fn update_user_handler(req: Request) -> Result<Response> {
    let input = req.json::<UserInput>()?;
    validate_user_input(&input)?;
    let user = db::update_user(req.user_id, input).await?;
    Ok(Response::json(user))
}
// 3 lines duplicated, but intent is immediately clear
```

❌ **BAD - "Clever" code (hard to understand)**:
```rust
let result = items.iter().fold(HashMap::new(), |mut acc, item| {
    *acc.entry(item.category).or_insert(0) += item.value;
    acc
});
```

✅ **GOOD - Explicit code (easy to understand)**:
```rust
let mut totals = HashMap::new();
for item in items {
    let current = totals.get(&item.category).unwrap_or(&0);
    totals.insert(item.category, current + item.value);
}
```

**Why This Matters**:
Failing to perform thorough self-review and write simple, clear code results in:
- ❌ Wasted verification cycles (verification will catch incomplete work)
- ❌ Main Agent spawns fix agents unnecessarily
- ❌ Specification gets polluted with urgent fix tasks
- ❌ Complex code leads to bugs and maintenance nightmares
- ❌ Future agents waste time understanding convoluted logic
- ❌ Over-abstraction makes debugging harder
- ❌ **USER WILL SHOUT AT YOU** for sloppy, confusing, or overly complex code
- ❌ Lost time and productivity

**Remember**:
- Verification agents check code quality and standards, but YOU are responsible for ensuring your implementation is complete, correct, and meets the requirements.
- **Simple, clear code is more maintainable than clever, abstracted code**
- **If you wouldn't understand your code in 6 months, simplify it now**

#### Learning Documentation Requirements

**MANDATORY**: After completing implementation work related to a specification, agents **MUST** document learnings.

**Documentation Style Requirements** (CRITICAL):
- ✅ **Clear and concise**: Use simple, direct language
- ✅ **Summarized**: Focus on key insights, not verbose explanations
- ✅ **Quick to scan**: Use bullet points, short sentences (1-2 lines max)
- ✅ **Effective for context management**: Keep learnings brief enough to be easily consumed by future agents
- ✅ **Concrete examples**: Show actual code snippets (2-5 lines) rather than long descriptions
- ❌ **AVOID verbosity**: No lengthy paragraphs, no obvious statements, no excessive detail
- ❌ **AVOID obtuseness**: No complex jargon without context, no abstract concepts without examples

**Quality Check**:
```
Ask yourself: Can I understand this learning in 5 seconds?
├─ YES → Good documentation ✅
└─ NO  → Too verbose, simplify it ❌
```

**Two Types of Learning Documentation**:

##### 1. Specification-Specific Learnings

**Location**: `.agents/specifications/[NN-spec-name]/learnings.md`

**Purpose**: Document critical knowledge specific to this specification that will help with:
- Future work on this specification
- Debugging issues that arise
- Understanding why certain decisions were made
- Avoiding repeating mistakes

**What to Document**:
```markdown
# Learnings - [Specification Name]

## Critical Implementation Details
- Auth token must validate BEFORE rate limiter (prevents token leakage)
- DB pool: exactly 20 connections (downstream service limit)
- Images must be 512x512 before S3 upload (cost optimization)

## Common Failures and Fixes
- Error: `connection timeout` → Increase pool size from 10 to 20
- Test failure: Mock timing issue → Use `tokio::time::pause()` for deterministic tests
- Build failed: Missing feature flag → Add `features = ["json"]` to Cargo.toml

## Dependencies and Interactions
- Uses `jsonwebtoken` v8.3 (v9 has breaking changes, avoid)
- Triggers webhook AFTER db commit (order matters for consistency)
- Requires env var `SECRET_KEY` (min 32 chars, validation in main.rs:45)

## Testing Insights
- Must test token expiry edge case (expires at exactly midnight fails)
- Use `#[serial]` for db tests (parallel tests cause conflicts)
- Mock S3 with `aws-sdk-s3::Config::builder().endpoint_url()`

## Future Considerations
- TODO: Add connection pooling retry logic (currently fails fast)
- Tech debt: Hardcoded 512x512 size (should be configurable)
- Scale: Current design supports <10k users (needs sharding beyond that)
```

**Format Guidelines**:
- Each entry: 1-2 lines maximum
- Use `→` for cause-effect relationships
- Include file:line references when relevant
- Show actual values/code rather than describing them

**When to Create/Update**:
- Create learnings.md on first task completion for a specification
- Update after each task if new critical insights are gained
- Update after any verification failures (document what was learned)

##### 2. Stack-Specific Generic Learnings

**Location**: `.agents/stacks/[stack].md` (e.g., `.agents/stacks/rust.md`, `.agents/stacks/typescript.md`)

**Purpose**: Document programming language/stack knowledge that is:
- Generic enough to apply across multiple projects
- Not specific to one specification
- Useful for future agents working in this stack
- Best practices, patterns, or common pitfalls

**What to Document in Stack Files**:
```markdown
# [Language] Stack - Learning Log

## Generic Patterns That Work Well
- Use `?` operator for error propagation (not `unwrap()` or `expect()`)
- `Arc<Mutex<T>>` for shared mutable state in async (not `Rc<RefCell<T>>`)
- Pattern: `let handle = tokio::spawn(async move { ... }); handle.await??` for task errors

## Common Pitfalls to Avoid
- `tokio::spawn()` doesn't propagate panics → wrap in `Result` and handle explicitly
- `.clone()` on `Arc` is cheap (ref count), on `Vec` is expensive (deep copy)
- Deadlock: Never `.lock()` same `Mutex` twice in one function

## Testing Best Practices
- Use `#[tokio::test]` for async tests (not `#[test]`)
- Mock external services with `mockito` crate: `mockito::mock("GET", "/api")`
- Pattern: `assert_matches!(result, Err(ErrorType::Specific))` for error testing

## Tooling Tips
- `cargo clippy -- -D warnings` catches most issues (zero warnings = required)
- `cargo expand` shows macro output (useful for debugging derive macros)
- `RUST_BACKTRACE=1` for stack traces, `RUST_LOG=debug` for tracing logs
```

**Format Guidelines**:
- Each entry: 1 line with optional code example (keep code under 5 lines)
- Use concrete examples, not abstract explanations
- Show the "right way" with code, not just prose
- Include actual command flags/syntax when relevant

**Decision Tree for Where to Document**:
```
Is this learning specific to this specification/feature?
├─ YES → Document in .agents/specifications/[NN-spec-name]/learnings.md
└─ NO  → Is it generic knowledge about the programming language/stack?
          ├─ YES → Document in .agents/stacks/[stack].md
          └─ NO  → Don't document (too trivial or obvious)
```

**Examples**:

**Specification-Specific** (goes in `learnings.md`):
- ✅ GOOD: "Auth token validates BEFORE rate limiter (prevents token leakage)"
- ❌ BAD: "We decided to implement the authentication token validation step before the rate limiting middleware is executed because this prevents a potential security vulnerability where tokens could leak through the rate limiter"
- ✅ GOOD: "DB pool: 20 connections (downstream limit) - see config.rs:34"
- ❌ BAD: "The database connection pool has been configured to use exactly twenty connections because the downstream service has limitations"

**Stack-Generic** (goes in `.agents/stacks/rust.md`):
- ✅ GOOD: "Use `?` for error propagation (not `unwrap()` in production)"
- ❌ BAD: "You should always use the question mark operator for error propagation instead of using unwrap() or expect() because these can cause panics in production code"
- ✅ GOOD: "`tokio::spawn()` doesn't propagate panics → wrap in `Result` explicitly"
- ❌ BAD: "When using tokio::spawn() to spawn asynchronous tasks, it's important to understand that panics won't be propagated automatically to the caller"

**Why Conciseness Matters**:
- Future agents need to scan 10-50 learnings quickly
- Verbose docs increase token usage and slow down context processing
- Key insight gets lost in unnecessary words
- Clear, direct language is more actionable

**Learning Documentation Workflow**:

1. Implementation agent completes work
2. Agent performs self-review (see above)
3. Agent **evaluates if learnings should be documented**:
   - Did I discover something important that would help future work?
   - Did I encounter a failure that taught me something critical?
   - Did I make a non-obvious design decision?
   - Is there a gotcha that future agents should know about?
4. If YES to any above:
   - Determine if learning is specification-specific or stack-generic
   - Create/update appropriate learnings.md or stack.md file
   - Document clearly and concisely
5. Report completion to Main Agent (including mention of learnings documented)

**Implementation Agent MUST NOT Skip Learning Documentation**:
- Documentation helps future agents succeed
- Prevents repeating mistakes
- Builds institutional knowledge
- Reduces verification failures
- **USER EXPECTS quality documentation**

**Implementation Agent MUST Write Concise Documentation**:
- ❌ Verbose documentation wastes tokens and time
- ❌ Long paragraphs hide critical insights
- ✅ 1-2 line entries are quick to scan and highly actionable
- ✅ Code examples (2-5 lines) are better than prose explanations
- ✅ Direct language makes learnings immediately useful
- **USER WILL BE FRUSTRATED** by verbose, hard-to-scan documentation

### Phase 2: Mandatory Verification (IRON-CLAD)

#### Main Agent Verification Orchestration

After receiving completion report from implementation agent, Main Agent **MUST**:

**Step 1: Identify Languages**
```
Main Agent analyzes changed files:
- Determine which language(s) were modified
- Identify relevant stack files (.agents/stacks/[language].md)
- Determine which verification agents to launch
```

**Step 2: Launch Verification Agents**

**CRITICAL RULE**: ONE verification agent per language stack (NEVER more than one per stack)

```
For each language modified:
  Main Agent spawns ONE [Language] Verification Agent

  Example:
  - Rust files changed → Spawn ONE Rust Verification Agent
  - TypeScript files changed → Spawn ONE JavaScript Verification Agent
  - Python files changed → Spawn ONE Python Verification Agent

  NEVER spawn multiple verification agents for the same language
  (prevents race conditions and file conflicts)
```

**Step 3: Provide Context to Verification Agents**

Main Agent provides each verification agent with:
```
Context Package:
- Changed files list (filtered by language)
- Implementation description
- Specification reference (.agents/specifications/NN-name/)
- Requirements.md location
- Tasks.md location
- Expected behavior
```

#### Verification Agent Execution

Each verification agent **MUST** execute ALL checks for their language:

##### Rust Verification Agent
**Must run in order**:
1. ✅ `cargo fmt -- --check` - Format verification
2. ✅ `cargo clippy -- -D warnings` - Lint (zero warnings)
3. ✅ `cargo test` - Run ALL tests OR specific crate tests
   ```bash
   # All tests
   cargo test

   # OR specific crate (if changes localized)
   cargo test --package crate-name
   ```
4. ✅ `cargo build` - Ensure successful build
   ```bash
   cargo build --all-features
   ```
5. ✅ `cargo doc --no-deps` - Documentation build
6. ✅ `cargo audit` - Security check
7. ✅ Standards compliance checks (see `.agents/stacks/rust.md`)

**On Success**: Report PASS to Main Agent
**On Failure**: Report FAIL with detailed errors to Main Agent

##### JavaScript/TypeScript Verification Agent
**Must run in order**:
1. ✅ `prettier --check .` - Format verification
2. ✅ `tsc --noEmit` - Type check (zero errors)
3. ✅ `eslint --max-warnings 0` - Lint (zero warnings)
4. ✅ `npm test` - Run tests with coverage
5. ✅ `npm run build` - Ensure successful build
6. ✅ `npm audit` - Security check
7. ✅ Standards compliance checks (see `.agents/stacks/javascript.md`)

**On Success**: Report PASS to Main Agent
**On Failure**: Report FAIL with detailed errors to Main Agent

##### Python Verification Agent
**Must run in order**:
1. ✅ `black --check .` - Format verification
2. ✅ `ruff check .` - Lint (zero errors)
3. ✅ `mypy .` - Type check (strict mode)
4. ✅ `pytest --cov` - Run tests with coverage
5. ✅ `python -m py_compile src/**/*.py` - Import check
6. ✅ `pip-audit` or `bandit` - Security check
7. ✅ Standards compliance checks (see `.agents/stacks/python.md`)

**On Success**: Report PASS to Main Agent
**On Failure**: Report FAIL with detailed errors to Main Agent

#### Verification Report Format

Each verification agent returns:
```markdown
# [Language] Verification Report

## Status: PASS ✅ / FAIL ❌

## Files Verified
- [list of files checked]

## Check Results
1. Format: PASS ✅ / FAIL ❌
2. Lint: PASS ✅ / FAIL ❌
3. Type Check: PASS ✅ / FAIL ❌
4. Tests: PASS ✅ / FAIL ❌ ([N] passed, [N] failed)
5. Build: PASS ✅ / FAIL ❌
6. Security: PASS ✅ / FAIL ❌
7. Standards: PASS ✅ / FAIL ❌

## Details
[Specific errors, warnings, or issues if any]

## Test Results
- Total: [N]
- Passed: [N]
- Failed: [N]
- Coverage: [N]%

## Blockers
[Issues preventing commit, if any]

## Recommendations
[Suggestions for improvement, if any]
```

### Phase 3: Main Agent Decision

Main Agent receives verification reports and **MUST** follow this logic:

#### If ALL Verifications PASS ✅

```
Main Agent MUST:
1. ✅ Receive PASS reports from all verification agents
2. ✅ Identify related specification (if applicable)
3. ✅ Spawn Specification Update Agent (NEVER update directly)
4. ✅ Provide Specification Agent with:
   - Verification report (PASS status)
   - Completed tasks list
   - Files changed
   - Implementation description
5. ✅ WAIT for Specification Agent to complete
6. ✅ Commit code AND specification updates following Rule 03
7. ✅ Include verification status in commit message:
      "Verified by [Language] Verification Agent(s): All checks passed"
8. ✅ Push to remote following Rule 05
9. ✅ Report success to user
```

##### Specification Update Process

**If work relates to a specification**:

```
Main Agent responsibilities:
1. Identify related specification directory
2. Spawn Specification Update Agent
3. Provide agent with:
   - Specification path (.agents/specifications/NN-spec-name/)
   - Verification report (full PASS report)
   - List of tasks that were completed
   - Implementation summary
4. WAIT for agent to complete updates
5. Review agent's completion report
6. Commit specification changes with code

Main Agent MUST NOT:
❌ Read tasks.md directly
❌ Update tasks.md directly
❌ Update frontmatter directly
❌ Mark tasks as complete directly
```

**Specification Update Agent responsibilities**:

```
Specification Update Agent MUST:
1. Read tasks.md from specification directory
2. Identify which tasks were completed (from Main Agent's context)
3. Mark tasks as [x] completed
4. Update frontmatter:
   - Increment 'completed' count
   - Decrement 'uncompleted' count
5. Add completion notes if needed
6. Save tasks.md
7. Report completion to Main Agent
```

**Example tasks.md update**:
```markdown
---
completed: 7  # was 5, now 7
uncompleted: 3  # was 5, now 3
tools:
  - Rust
  - Cargo
  - Clippy
---

# Feature Implementation - Tasks

## Implementation Tasks
- [x] Create base API structure
- [x] Implement authentication endpoint  ← Just completed
- [x] Add error handling  ← Just completed
- [ ] Add rate limiting
- [ ] Add monitoring
- [ ] Write integration tests
```

#### If ANY Verification FAILS ❌

```
Main Agent MUST:
1. ❌ Receive FAIL report from one or more verification agents
2. ❌ DO NOT COMMIT any code
3. ❌ DO NOT push to remote
4. ✅ Identify which verification(s) failed
5. ✅ Extract detailed error information from verification report
6. ✅ Identify related specification directory
7. ✅ Spawn Specification Update Agent (NEVER update directly)
8. ✅ Provide Specification Agent with:
   - Specification path (.agents/specifications/NN-spec-name/)
   - Full verification FAIL report
   - Failed checks details
   - Files affected
   - Recommendation for fix
9. ✅ WAIT for Specification Agent to create urgent task and verification.md
10. ✅ Report detailed failures to user
11. ✅ Provide fix recommendations

Main Agent MUST NOT:
❌ Update tasks.md directly
❌ Create verification.md directly
❌ Update frontmatter directly
❌ Add urgent tasks directly
```

##### Failed Verification Task Creation

**When verification fails**, Main Agent **MUST** delegate to Specification Update Agent.

**Specification Update Agent responsibilities when verification FAILS**:

```
Specification Update Agent MUST:
1. Read tasks.md from specification directory
2. Create verification.md file in same directory with full verification report:
   - Status: FAIL
   - Date/time of failure
   - Language(s) that failed
   - All failed checks with details
   - Error messages and line numbers
   - Files affected
   - Recommended fixes
3. Add NEW URGENT task at TOP of tasks.md:
   - Mark as highest priority
   - Reference verification.md for details
   - Include brief summary of issues
4. Update frontmatter:
   - Increment 'uncompleted' count by 1
5. Save tasks.md
6. Save verification.md
7. Report completion to Main Agent

IMPORTANT: verification.md is TRANSIENT:
- Created fresh each verification failure
- Overwritten on next verification failure
- Deleted on verification success
- Agent reads it to understand what to fix
```

**Example verification.md**:
```markdown
# Verification Report - FAILED

**Status**: FAIL ❌
**Date**: 2026-01-11 15:30:45
**Language**: Rust
**Specification**: .agents/specifications/03-user-authentication/

## Failed Checks

### 1. Clippy Lint - FAIL ❌
```
warning: using `unwrap()` on a `Result` value
  --> src/auth/token.rs:45:22
   |
45 |     let key = key_result.unwrap();
   |                          ^^^^^^^^
   |
   = note: `#[warn(clippy::unwrap_used)]` on by default
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#unwrap_used

error: could not compile `auth-service` due to previous error; 2 warnings emitted
```

### 2. Tests - FAIL ❌
```
running 45 tests
test auth::test_generate_token ... ok
test auth::test_validate_token_expired ... FAILED
test auth::test_validate_token_invalid ... FAILED
...

failures:

---- auth::test_validate_token_expired stdout ----
thread 'auth::test_validate_token_expired' panicked at 'assertion failed: `(left == right)`
  left: `true`,
 right: `false`', tests/auth_tests.rs:67:5

---- auth::test_validate_token_invalid stdout ----
thread 'auth::test_validate_token_invalid' panicked at 'called `Result::unwrap()` on an `Err` value: InvalidToken', tests/auth_tests.rs:82:37

test result: FAILED. 43 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files Affected
- src/auth/token.rs (line 45: unwrap() usage)
- tests/auth_tests.rs (line 67: wrong assertion)
- tests/auth_tests.rs (line 82: unwrap() in test)

## Recommended Fixes

1. **Replace unwrap() with proper error handling**:
   ```rust
   // Change from:
   let key = key_result.unwrap();

   // To:
   let key = key_result?;  // or handle error explicitly
   ```

2. **Fix test assertion in test_validate_token_expired**:
   - Expected result was inverted
   - Update assertion at line 67

3. **Fix test panic in test_validate_token_invalid**:
   - Should use `unwrap_err()` since we expect an error
   - Or use pattern matching

## Agent Action Required

Read this verification report, fix all issues listed above, then:
1. Mark the urgent fix task in tasks.md as complete
2. Continue with remaining tasks in tasks.md
3. Report completion to Main Agent
4. Main Agent will re-run verification

When verification passes, this verification.md file will be deleted.
```

**Example tasks.md with urgent task**:
```markdown
---
completed: 5
uncompleted: 6  # Incremented by 1 for urgent task
tools:
  - Rust
  - Cargo
  - Clippy
---

# User Authentication - Tasks

## URGENT: Failed Verification Tasks
- [ ] **FIX: Verification failures in user authentication**
  - Verification failed on 2026-01-11 15:30
  - Language: Rust
  - Failed checks: Clippy (1 error), Tests (2 failures)
  - See verification.md for detailed report
  - Files affected: src/auth/token.rs, tests/auth_tests.rs
  - Must fix before proceeding

## Implementation Tasks
- [x] Create base API structure
- [x] Implement authentication endpoint
- [ ] Add rate limiting
- [ ] Add monitoring
- [ ] Write integration tests
```

**Notification to Implementation Agent or User**:
```
Main Agent reports:

❌ VERIFICATION FAILED ❌

Verification Agent: [Language] Verification Agent
Status: FAIL
Failed Checks:
  - [Check 1]: [Details]
  - [Check 2]: [Details]

Test Failures:
  - [Test 1]: [Error]
  - [Test 2]: [Error]

Code has NOT been committed.

Action Required:
1. Fix the issues listed above
2. Re-run implementation
3. Verification will run again automatically
4. Code will only be committed after verification passes

Specification updated: .agents/specifications/NN-name/tasks.md
New urgent task added at top of task list.
```

### Phase 4: Fix and Retry (If Verification Failed)

#### Implementation Agent Fixes Issues

```
1. Main Agent spawns Implementation Agent (or resumes existing agent)
2. Main Agent provides context:
   - Specification path
   - verification.md location
   - Urgent task details
3. Implementation agent reads verification.md:
   - Understands all failed checks
   - Reviews error messages and line numbers
   - Identifies recommended fixes
4. Implementation agent fixes code issues:
   - Addresses ALL failures listed in verification.md
   - Ensures tests pass locally
   - Follows all stack standards
5. Implementation agent marks urgent fix task as [x] complete in tasks.md
6. Implementation agent REPORTS completion to Main Agent again
7. Main Agent launches verification agents again (back to Phase 2)
8. IF verification PASSES:
   - Specification Update Agent deletes verification.md
   - Marks completed tasks in tasks.md
   - Main Agent commits all changes
9. IF verification FAILS again:
   - Process repeats (Specification Agent updates verification.md)
   - New urgent task created (or existing one updated)
   - Implementation agent fixes again
```

**CRITICAL**: This loop continues until ALL verifications PASS. There is NO bypass.

**verification.md Lifecycle**:
- Created by Specification Update Agent on verification FAIL
- Read by Implementation Agent to understand fixes needed
- Overwritten on subsequent failures
- Deleted by Specification Update Agent on verification PASS

## Verification Agent Rules (IRON-CLAD)

### Mandatory Requirements

Each verification agent **MUST**:
1. ✅ Be spawned by Main Agent ONLY
2. ✅ Run ALL checks for their language (no skipping)
3. ✅ Be the ONLY verification agent for that language stack
4. ✅ Generate comprehensive report
5. ✅ Report to Main Agent ONLY
6. ✅ NEVER commit code (Main Agent delegates this)
7. ✅ NEVER update specifications (Main Agent delegates this)
8. ✅ NEVER create verification.md (Main Agent delegates this)

Each verification agent **MUST NOT**:
1. ❌ Skip any verification checks
2. ❌ Allow partial passes ("tests mostly pass")
3. ❌ Commit code directly
4. ❌ Update tasks.md directly
5. ❌ Create verification.md directly
6. ❌ Run concurrently with another agent for same language
7. ❌ Proceed when checks fail

### Race Condition Prevention

**CRITICAL RULE**: Only ONE verification agent per language stack at any time.

```
Good ✅:
  Main Agent sees Rust changes
  → Spawns ONE Rust Verification Agent
  → Waits for completion
  → Agent finishes
  → Reports back

Bad ❌:
  Main Agent sees Rust changes
  → Spawns Rust Verification Agent #1
  → Also spawns Rust Verification Agent #2
  → VIOLATION: Race condition possible!
  → File conflicts
  → Inconsistent results
```

**Prevention Strategy**:
```
Main Agent tracks active verification agents:
  active_verifications = {
    'rust': null,      # No Rust verification running
    'javascript': null, # No JS verification running
    'python': null      # No Python verification running
  }

Before spawning verification agent:
  IF active_verifications[language] is not null:
    WAIT for existing agent to complete
  ELSE:
    Spawn new verification agent
    Mark active_verifications[language] = agent_id
```

## Complete Workflow Example

### Successful Workflow ✅

```
1. User: "Implement user authentication in Rust"

2. Main Agent:
   - Reads specification: .agents/specifications/03-user-authentication/
   - Reads requirements.md and tasks.md
   - Identifies tasks to complete

3. Main Agent:
   - Spawns Rust Implementation Agent
   - Provides context (spec, requirements, tasks)

4. Rust Implementation Agent:
   - Reads AGENTS.md
   - Reads all rules from .agents/rules/*
   - Reads .agents/stacks/rust.md
   - Reads specification files
   - Implements authentication code
   - Writes tests
   - REPORTS completion to Main Agent:
     * Files changed: [list]
     * Language: Rust
     * Description: [what was built]
     * Specification: 03-user-authentication
   - STOPS and WAITS

5. Main Agent:
   - Receives completion report
   - Identifies language: Rust
   - Checks active_verifications['rust'] = null ✅
   - Spawns ONE Rust Verification Agent
   - Marks active_verifications['rust'] = agent-id-123
   - Provides context (files, spec, description)

6. Rust Verification Agent:
   - Reads .agents/stacks/rust.md
   - Runs cargo fmt -- --check → PASS ✅
   - Runs cargo clippy -- -D warnings → PASS ✅
   - Runs cargo test → PASS ✅ (45 tests passed)
   - Runs cargo build → PASS ✅
   - Runs cargo doc → PASS ✅
   - Runs cargo audit → PASS ✅
   - Checks standards compliance → PASS ✅
   - Generates comprehensive report
   - Reports to Main Agent: PASS ✅

7. Main Agent:
   - Receives PASS report
   - Marks active_verifications['rust'] = null
   - Identifies specification: .agents/specifications/03-user-authentication/
   - Spawns Specification Update Agent
   - Provides context:
     * Verification report (PASS)
     * Completed tasks: authentication middleware, JWT generation, tests
     * Files changed: [list]

8. Specification Update Agent:
   - Reads .agents/specifications/03-user-authentication/tasks.md
   - Marks completed tasks as [x]:
     * [x] Implement authentication middleware
     * [x] Add JWT token generation
     * [x] Write authentication tests
   - Updates frontmatter: completed: 8, uncompleted: 2
   - Deletes verification.md if it exists (cleanup from previous failure)
   - Saves tasks.md
   - Reports completion to Main Agent

9. Main Agent:
   - Receives completion report from Specification Agent
   - git add [all changed files including specification]
   - git commit with message:
     ```
     Implement user authentication middleware

     Added JWT token generation and validation with authentication
     middleware for securing API endpoints.

     Changes made:
     - Implemented JWT token generation and validation
     - Created authentication middleware
     - Added comprehensive tests (45 tests, all passing)
     - Updated error handling for auth failures

     Verified by Rust Verification Agent: All checks passed
     - Format: PASS (rustfmt)
     - Lint: PASS (clippy, 0 warnings)
     - Tests: 45/45 PASS
     - Build: PASS (debug and release)
     - Doc: PASS (cargo doc)
     - Security: PASS (cargo audit)
     - Coverage: 89%

     Specification: .agents/specifications/03-user-authentication/
     Tasks completed: 3
     Tasks remaining: 2

     Co-Authored-By: Claude <noreply@anthropic.com>
     ```
   - git status (verify)
   - git push (automatic, Rule 05)

10. Main Agent:
   - Reports success to user:
     ```
     ✅ User authentication implemented successfully!

     Verification Status: PASS
     - All 45 tests passing
     - Code coverage: 89%
     - Zero lint warnings
     - Build successful (debug and release)
     - Documentation complete
     - No security vulnerabilities

     Specification updated: 3 tasks completed, 2 remaining

     Changes pushed to remote: foundation_ai branch
     ```
```

### Failed Verification Workflow ❌

```
1. User: "Add data validation to API endpoints"

2. Main Agent:
   - Reads specification: .agents/specifications/05-data-validation/
   - Spawns Python Implementation Agent

3. Python Implementation Agent:
   - Implements validation logic
   - Writes some tests (but not all edge cases)
   - REPORTS completion to Main Agent

4. Main Agent:
   - Spawns ONE Python Verification Agent

5. Python Verification Agent:
   - Runs black -- --check → PASS ✅
   - Runs ruff check . → FAIL ❌ (3 lint errors)
   - Runs mypy . → FAIL ❌ (missing type hints)
   - Runs pytest → FAIL ❌ (2 tests failing)
   - Stops checking (failures detected)
   - Generates FAIL report with details

6. Main Agent:
   - Receives FAIL report
   - DOES NOT COMMIT code ❌
   - Identifies specification: .agents/specifications/05-data-validation/
   - Spawns Specification Update Agent
   - Provides context:
     * Verification report (FAIL with full details)
     * Failed checks summary
     * Files affected: src/validation.py, tests/test_validation.py
     * Recommended fixes

7. Specification Update Agent:
   - Reads .agents/specifications/05-data-validation/tasks.md
   - Creates verification.md with full FAIL report:
     * All lint errors with line numbers
     * All type errors
     * All test failures with stack traces
     * Recommended fixes
   - Adds NEW URGENT task at TOP of tasks.md:
     ```markdown
     ## URGENT: Failed Verification Tasks
     - [ ] **FIX: Verification failures in data validation**
       - Verification failed on 2026-01-11 15:30
       - Language: Python
       - Failed checks: Lint (3 errors), Type Check (5 errors), Tests (2 failures)
       - See verification.md for detailed report
       - Files affected: src/validation.py, tests/test_validation.py
       - Must fix before proceeding
     ```
   - Updates frontmatter: uncompleted: 4 → 5
   - Saves tasks.md and verification.md
   - Reports completion to Main Agent

8. Main Agent:
   - Receives completion report
   - DOES NOT commit anything ❌
   - DOES NOT push anything ❌
   - Reports to user:
     ```
     ❌ VERIFICATION FAILED ❌

     Python Verification Agent found issues:

     Lint Errors (3):
     - Line 45: Unused import 're'
     - Line 103: Line too long (115 > 100 characters)
     - Line 67: Undefined variable 'pattern'

     Type Errors (5):
     - Function 'validate_email' missing return type hint
     - Function 'validate_phone' missing parameter type hints
     [...]

     Test Failures (2):
     - test_email_validation: Expected True, got False
     - test_phone_validation: KeyError: 'country_code'

     Code has NOT been committed.

     Action Required:
     1. Fix the 3 lint errors
     2. Add missing type hints (5 functions)
     3. Fix the 2 failing tests
     4. Re-submit for verification

     Specification updated: urgent task added, verification.md created
     Full details: .agents/specifications/05-data-validation/verification.md
     ```

9. Main Agent:
   - Spawns Implementation Agent (or resumes existing)
   - Provides context:
     * Specification path
     * verification.md location
     * Urgent task to complete

10. Implementation Agent:
   - Reads verification.md
   - Fixes all lint errors
   - Adds all missing type hints
   - Fixes both failing tests
   - Marks urgent fix task as [x] in tasks.md
   - REPORTS completion to Main Agent again

11. Main Agent:
   - Spawns Python Verification Agent again
   - Agent runs all checks
   - All checks PASS ✅

12. Main Agent:
   - Spawns Specification Update Agent
   - Provides context (PASS report, completed tasks)

13. Specification Update Agent:
   - Marks urgent fix task as [x] completed
   - Marks original validation tasks as [x] completed
   - Deletes verification.md (verification passed)
   - Updates frontmatter
   - Saves tasks.md
   - Reports completion

14. Main Agent:
   - Commits code and specification updates
   - Pushes to remote (Rule 05)
   - Reports success to user
```

## Integration with Other Rules

### Rule 03 (Work Commit Rules)
- Verification happens BEFORE commit
- Commit message includes verification status
- Only verified code is committed
- Specification updates included in commit

### Rule 05 (Git Auto-Approval and Push)
- Push happens AFTER successful verification
- No manual approval needed (verification is the gate)
- Automatic push on verification pass

### Rule 06 (Specifications and Requirements)
- Verification agent receives specification context
- Tasks.md updated based on verification results
- Failed verification creates urgent task
- Successful verification marks tasks complete

### Rule 07 (Language Conventions)
- Verification enforces all stack standards
- Stack files define verification commands
- Learning Logs updated with issues found
- Standards continuously improved

## Enforcement (ZERO TOLERANCE)

### Violations

The following are **CRITICAL VIOLATIONS** with **ZERO TOLERANCE**:

1. ❌ **Implementation agent commits directly** (bypasses verification)
2. ❌ **Main agent commits without verification** (skips quality gate)
3. ❌ **Verification agent skips checks** (incomplete verification)
4. ❌ **Multiple verification agents for same language** (race condition)
5. ❌ **Committing code with failed verification** (quality breach)
6. ❌ **Not updating specification on verification failure** (lost tracking)
7. ❌ **Sub-agent spawns verification agent** (violates authority hierarchy)

### Consequences

Any violation results in:
1. **IMMEDIATE REVERT** of any committed code
2. **STOP ALL WORK** until violation is corrected
3. **DOCUMENT in Learning Log** (violation details)
4. **REPORT to user** (transparency)
5. **RE-RUN proper workflow** (correct process)

### User Impact

Violations have severe consequences:
- ❌ **Broken builds** in production
- ❌ **Failed tests** discovered too late
- ❌ **Race conditions** from concurrent verifications
- ❌ **Lost work** from reverts
- ❌ **Wasted time** fixing avoidable issues
- ❌ **User frustration** and lost trust

**THE USER WILL BE VERY UPSET** if this workflow is not followed!

## Summary

**Core Workflow** (IRON-CLAD):
```
Implement (TDD: Test → Red → Code → Green → Refactor) → Self-Review → Document Learnings → Report → Verify → Update Spec → Commit → Push
```

**Key Rules**:
1. ✅ Implementation agents NEVER commit directly
2. ✅ **Implementation agents MUST follow TDD: Write tests FIRST, verify failure, then implement**
3. ✅ Implementation agents MUST perform self-review before reporting completion
4. ✅ Implementation agents MUST document learnings (specification-specific or stack-generic)
5. ✅ Main Agent ALWAYS delegates to verification
6. ✅ ONE verification agent per language stack (no more)
7. ✅ ALL checks must PASS before commit
8. ✅ Specifications updated based on verification results
9. ✅ Failed verification creates urgent task
10. ✅ Process repeats until verification passes
11. ✅ **ONLY Main Agent can spawn verification agents** (sub-agents cannot)

**Critical Self-Review Requirements**:
- ✅ **Check TDD followed: Tests written first, verified failing, then implementation**
- ✅ Check completeness: No partial implementations or fake code
- ✅ Check code quality: Clear logic, proper error handling
- ✅ **Check code simplicity: Can it be simplified? Break down nested logic (max 2-3 levels)**
- ✅ **DRY vs Clarity: Prefer clarity - OK to duplicate 2-5 lines if abstraction adds complexity**
- ✅ **Code reads like prose: Explicit > clever, straightforward > "smart tricks"**
- ✅ Check requirements alignment: Verify against tasks.md and requirements.md
- ✅ Check test coverage: Meaningful tests for all functionality
- ✅ **Check test documentation: Every test has WHY/WHAT comments (2-5 lines)**
- ❌ **USER WILL SHOUT AT YOU** if you skip self-review and submit incomplete work
- ❌ **USER WILL SHOUT AT YOU** if you don't think hard about simplification
- ❌ **USER WILL SHOUT AT YOU** for tests without documentation
- ❌ **USER WILL SHOUT AT YOU** for writing implementation before tests

**Learning Documentation Requirements**:
- ✅ Create/update `.agents/specifications/[NN-spec-name]/learnings.md` for specification-specific insights
- ✅ Update `.agents/stacks/[stack].md` for generic programming language learnings
- ✅ Document critical decisions, gotchas, failures, and solutions
- ✅ Help future agents avoid mistakes and succeed faster
- ✅ **Keep entries concise: 1-2 lines max, use code examples over prose**
- ✅ **Quick 5-second scan test: Can it be understood immediately?**
- ✅ **Task-specific test details go in test comments, NOT learnings.md**
- ❌ **NO verbose paragraphs, NO obvious statements, NO excessive detail**
- ❌ **NO polluting learnings.md with individual test explanations**
- ❌ **USER WILL BE FRUSTRATED** by verbose, hard-to-scan documentation

**Zero Tolerance**:
- ❌ No bypassing verification
- ❌ No skipping self-review
- ❌ No skipping TDD (write tests first!)
- ❌ No implementing before writing tests
- ❌ No incomplete implementations
- ❌ No fake or placeholder code
- ❌ No tests without WHY/WHAT documentation
- ❌ No overly nested or complex code (think hard about simplification!)
- ❌ No "clever" code that sacrifices clarity
- ❌ No forced abstractions that add complexity
- ❌ No skipping checks
- ❌ No partial passes
- ❌ No concurrent verifications per stack
- ❌ No committing on failure

**Result**: **100% VERIFIED, COMPLETE, SIMPLE, WELL-DOCUMENTED, TEST-DRIVEN CODE** - Every commit is guaranteed to:
- Pass all quality gates
- Meet all requirements completely
- Be developed using TDD (tests written first, implementation driven by failing tests)
- Be simple, clear, and easy to understand (no unnecessary complexity)
- Use appropriate abstractions (DRY when it helps, inline when it clarifies)
- Include meaningful tests with WHY/WHAT documentation
- Preserve learnings for future work (bigger picture, not test-by-test details)

---
*Created: 2026-01-11*
*Last Updated: 2026-01-13 (Added TDD requirement, test documentation, code simplicity, and conciseness)*
