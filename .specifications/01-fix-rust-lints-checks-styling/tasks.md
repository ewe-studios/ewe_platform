---
completed: 0
uncompleted: 32
tools:
  - cargo clippy
  - cargo fmt
  - cargo test
  - cargo build
  - ripgrep (rg)
  - Rust Verification Agent
---

# Fix Rust Lints, Checks, and Styling - Tasks

## Task List

### Phase 1: Discovery and Assessment
- [ ] Run full clippy analysis on all workspace members and document all warnings
- [ ] Run cargo fmt check to identify all formatting issues
- [ ] Search codebase for unwrap() and expect() calls in production code
- [ ] Create detailed inventory of all issues by category and severity
- [ ] Prioritize issues based on severity and impact

### Phase 2: Critical Clippy Warnings
- [ ] Fix cast_possible_truncation warnings in foundation_nostd (u64 to usize conversions)
- [ ] Fix unnecessary_debug_formatting warnings in build.rs files
- [ ] Fix match_same_arms warnings in template-macro
- [ ] Fix unnecessary_wraps warnings for functions with unnecessarily wrapped Results
- [ ] Fix similar_names warnings for confusingly similar variable names

### Phase 3: Documentation Warnings
- [ ] Add # Errors sections to all public functions returning Result (missing_errors_doc)
- [ ] Add # Panics sections to functions that may panic (missing_panics_doc)
- [ ] Review and enhance existing documentation for clarity
- [ ] Add code examples to documentation where appropriate
- [ ] Verify all public items have proper documentation

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

### Phase 6: Backend Crates
- [ ] Fix all issues in foundation_core
- [ ] Fix all issues in foundation_wasm
- [ ] Fix all issues in foundation_nostd
- [ ] Fix all issues in foundation_macros
- [ ] Fix all issues in foundation_runtimes
- [ ] Fix all issues in foundation_ai

### Phase 7: Main Crates
- [ ] Fix all issues in crates/* directory (html, routing, templates, etc.)
- [ ] Fix all issues in bin/platform
- [ ] Fix all issues in infrastructure/* crates
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
- `backends/foundation_core/src/io/ioutils/mod.rs` (trailing whitespace)
- Multiple files with missing documentation

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
