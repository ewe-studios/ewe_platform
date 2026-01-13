---
description: Fix all pending Rust lints, checks, and styling mistakes across the codebase
status: uncompleted
---

# Fix Rust Lints, Checks, and Styling - Requirements

## Overview
This specification covers the systematic resolution of all pending Rust lints, checks, and styling mistakes across the entire ewe_platform codebase. The goal is to achieve zero warnings from cargo clippy, zero formatting issues from cargo fmt, and full compliance with the Rust coding standards defined in `.agents/stacks/rust.md`.

## Requirements Conversation Summary

### User Request
User requested the creation of a new specification for fixing all pending Rust lints, checks, and styling mistakes in the codebase.

### Clarifying Questions
Agent identified the following areas to address:
- What types of issues need to be fixed?
- Should this include all workspace members?
- What is the priority order for fixes?
- Should existing functionality be preserved?
- Are there any acceptable exceptions?

### User Responses
Based on the user request and codebase analysis:
- All clippy warnings must be resolved (zero warnings policy)
- All formatting issues must be corrected
- All workspace members should be included
- Existing functionality must be preserved (no breaking changes)
- Follow the standards defined in `.agents/stacks/rust.md`
- Priority: Critical warnings first, then pedantic warnings, then formatting

### Final Requirements Agreement
Create a comprehensive specification to systematically fix all Rust linting, checking, and styling issues across the entire ewe_platform workspace, ensuring full compliance with established Rust coding standards.

## Detailed Requirements

### Functional Requirements

1. **Clippy Lint Resolution**
   - Fix all clippy warnings with `-W clippy::all` and `-W clippy::pedantic`
   - Address specific warning categories:
     - `unnecessary_debug_formatting` in println! macros
     - `cast_possible_truncation` for u64 to usize conversions
     - `match_same_arms` for identical match branches
     - `missing_errors_doc` for functions returning Result
     - `missing_panics_doc` for functions that may panic
     - `unnecessary_wraps` for unnecessarily wrapped Results
     - `similar_names` for confusingly similar variable names
     - `module_name_repetitions` for redundant naming
     - `needless_pass_by_value` for arguments that should be references
     - `redundant_continue` for unnecessary continue expressions

2. **Formatting Corrections**
   - Fix trailing whitespace issues (e.g., `#[must_use] ` → `#[must_use]`)
   - Ensure consistent indentation
   - Apply rustfmt rules across all `.rs` files
   - Verify formatting with `cargo fmt -- --check`

3. **Documentation Improvements**
   - Add `# Errors` sections to all public functions returning `Result`
   - Add `# Panics` sections to functions that may panic
   - Ensure all public items have proper documentation
   - Include code examples in documentation where appropriate

4. **Code Quality Enhancements**
   - Replace `unwrap()` and `expect()` with proper error handling (excluding test code)
   - Use proper error propagation with `?` operator
   - Implement type-safe conversions (use `try_from` instead of `as` casts where appropriate)
   - Refactor match arms with identical bodies to use pattern alternatives

5. **Standards Compliance**
   - Ensure all code follows naming conventions (snake_case, PascalCase, SCREAMING_SNAKE_CASE)
   - Verify proper use of visibility modifiers
   - Check for proper trait implementations
   - Validate error handling patterns

### Non-Functional Requirements

1. **Performance**: Changes should not negatively impact runtime performance
2. **Safety**: All fixes must maintain or improve code safety
3. **Compatibility**: No breaking changes to public APIs
4. **Testability**: All existing tests must continue to pass
5. **Maintainability**: Code should be more maintainable after fixes

### Technical Specifications

- **Technology Stack**: Rust 1.87, Edition 2021
- **Tools Required**:
  - `cargo clippy` (with pedantic lints)
  - `cargo fmt` (with project rustfmt.toml)
  - `cargo test` (for validation)
  - `cargo build` (for compilation checks)
- **Workspace Structure**: Monorepo with multiple crates in:
  - `backends/*`
  - `bin/*`
  - `crates/*`
  - `infrastructure/*`
  - `examples/*`
  - `tests/*`
- **Integration Points**: All workspace members must be addressed

### Success Criteria

- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes with zero warnings
- [ ] `cargo fmt -- --check` passes with no formatting issues
- [ ] `cargo build --all-features` compiles successfully
- [ ] `cargo test --all-features` passes all tests
- [ ] All public APIs maintain backward compatibility
- [ ] Code quality metrics improve (fewer unwrap/expect, better documentation)
- [ ] No runtime behavior changes (existing functionality preserved)

## Important Notes for Agents

### Before Starting Work
- **MUST READ** both this requirements.md and tasks.md files
- **MUST VERIFY** completion status by searching the codebase
- **MUST UPDATE** tasks.md to reflect actual implementation status
- **MUST ADD** new tasks to tasks.md BEFORE starting work on them

### Verification Requirements
Agents cannot rely solely on the status field or task checkboxes. They **MUST**:
1. Run `cargo clippy` to identify all warnings
2. Run `cargo fmt --check` to identify formatting issues
3. Search the codebase for specific issue patterns (unwrap, expect, etc.)
4. Verify that code actually compiles and tests pass after changes
5. Update task status based on actual findings, not assumptions
6. Mark tasks as completed only when fully verified

### During Work
- Update tasks.md as you complete each task
- Add new tasks if you discover additional work needed
- Keep frontmatter counts accurate in tasks.md
- Update tools list as new tools are used
- Commit after each logical group of fixes
- Run verification checks before marking tasks complete

### Testing Strategy
- Run `cargo test` after each significant change
- Verify no test regressions
- Add new tests if behavior changes are necessary
- Use `cargo test --doc` to verify documentation examples

### Priority Order
1. **Critical**: Compilation errors (if any)
2. **High**: Clippy warnings that indicate potential bugs or safety issues
3. **Medium**: Documentation warnings (missing_errors_doc, missing_panics_doc)
4. **Low**: Style and formatting issues
5. **Enhancement**: Code quality improvements beyond warnings

### Change Management
- Make small, focused commits for each type of fix
- Group related changes together (e.g., all formatting in one commit)
- Write clear commit messages explaining what was fixed and why
- Preserve git history and blame information where possible

---
*Created: 2026-01-14*
*Last Updated: 2026-01-14*
