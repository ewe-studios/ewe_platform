---
completed: 10
uncompleted: 18
tools:
  - cargo clippy
  - cargo fmt
  - cargo test
  - cargo build
  - ripgrep (rg)
  - Rust Verification Agent
scope: Excludes foundation_core and infrastructure/* due to compilation errors
---

# Fix Rust Lints, Checks, and Styling - Tasks

**SCOPE NOTE:** This work excludes `backends/foundation_core` and `infrastructure/*` crates due to compilation errors. These will be addressed in a separate specification.

## Task List

### Phase 1: Discovery and Assessment
- [x] Run full clippy analysis on all workspace members and document all warnings
- [x] Run cargo fmt check to identify all formatting issues
- [x] Search codebase for unwrap() and expect() calls in production code
- [x] Create detailed inventory of all issues by category and severity
- [x] Prioritize issues based on severity and impact

### Phase 2: Critical Clippy Warnings
- [x] Fix cast_possible_truncation warnings in foundation_nostd (u64 to usize conversions)
- [ ] Fix unnecessary_wraps warnings for functions with unnecessarily wrapped Results
- [ ] Fix similar_names warnings for confusingly similar variable names

### Phase 3: Documentation and Style Warnings
- [x] Fix unnecessary_debug_formatting warnings in build.rs files
- [x] Fix match_same_arms warnings in template-macro
- [ ] Add # Errors sections to all public functions returning Result (missing_errors_doc)
- [ ] Add # Panics sections to functions that may panic (missing_panics_doc)
- [ ] Review and enhance existing documentation for clarity

### Phase 4: Code Quality Improvements
- [ ] Fix needless_pass_by_value warnings (use references instead of owned values)
- [ ] Fix redundant_continue expressions in watch_utils and other modules
- [ ] Fix module_name_repetitions warnings (e.g., field names starting with struct name)
- [ ] Replace direct unwrap()/expect() calls with proper error handling
- [ ] Implement try_from conversions instead of unsafe casts where appropriate

### Phase 5: Formatting Corrections
- [ ] Fix trailing whitespace in all source files
- [ ] Run cargo fmt on entire workspace
- [ ] Verify formatting consistency across all .rs files
- [ ] Fix any remaining formatting edge cases

### Phase 6: Backend Crates (Excluding foundation_core)
- [ ] Fix all issues in foundation_wasm
- [ ] Fix all issues in foundation_nostd
- [ ] Fix all issues in foundation_macros
- [ ] Fix all issues in foundation_runtimes
- [ ] Fix all issues in foundation_ai

### Phase 7: Main Crates (Excluding infrastructure)
- [ ] Fix all issues in crates/* directory (html, routing, templates, etc.)
- [ ] Fix all issues in bin/platform
- [ ] Fix all issues in examples/*
- [ ] Fix all issues in tests/*

### Phase 8: Verification and Validation
- [ ] Run cargo clippy --all-targets --all-features -- -D warnings (must pass)
- [ ] Run cargo fmt -- --check (must pass)
- [ ] Run cargo build --all-features (must compile)
- [ ] Run cargo test --all-features (all tests must pass)
- [ ] Run cargo doc --no-deps --all-features (docs must build)
- [ ] Verify no runtime behavior changes
- [ ] Launch Rust Verification Agent for final validation

## Notes

### Issue Categories Identified
From initial analysis, the following categories of issues were found:
1. **Formatting**: Trailing whitespace on `#[must_use]` attributes
2. **Casting**: u64 to usize truncation warnings
3. **Debug Formatting**: Unnecessary Debug formatting in println! macros
4. **Match Arms**: Identical match arms that should be merged
5. **Documentation**: Missing # Errors and # Panics sections
6. **Code Quality**: Redundant continue, similar names, unnecessary wraps
7. **Performance**: Arguments passed by value but not consumed

### Affected Files
Primary files needing attention (from initial scan):
- `backends/foundation_nostd/src/raw_parts.rs` (casting warnings)
- `bin/platform/build.rs` (debug formatting)
- `crates/template-macro/src/lib.rs` (match arms)
- `crates/watch_utils/src/lib.rs` (redundant continue)
- Multiple files with missing documentation

### Excluded from Scope
- `backends/foundation_core/*` - Has compilation errors (SSL imports, unstable features)
- `infrastructure/*` - Has build script failures
- These will be fixed in a separate specification

### Commit Strategy
- Phase 1: One commit for assessment documentation
- Phase 2-4: One commit per category of fixes
- Phase 5: One commit for all formatting
- Phase 6-7: One commit per major crate or logical grouping
- Phase 8: Final verification commit

### Testing Notes
- Run `cargo test` after each phase to catch regressions early
- Pay special attention to tests in modified modules
- Verify examples still compile and run correctly
- Check integration tests pass

---
*Last Updated: 2026-01-14*
