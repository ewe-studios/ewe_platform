# Project Specifications

## Overview
This directory contains all project specifications and requirements. Each specification represents a significant feature, enhancement, or change to the project.

## How Specifications Work

1. **Requirements-First**: Before work begins, main agent discusses requirements with user
2. **Documentation**: Requirements and tasks are documented in numbered specification directories
3. **User Approval**: User must explicitly approve and request implementation
4. **Agent Reading**: Agents MUST read requirements.md, tasks.md, and relevant feature files
5. **Status Verification**: Agents MUST verify completion status by searching the codebase
6. **Task Updates**: Agents MUST update tasks.md as work progresses
7. **Status Accuracy**: Agents MUST ensure status reflects actual implementation

## All Specifications

### [01: Fix Rust Lints, Checks, and Styling](./01-fix-rust-lints-checks-styling/)
**Status:** ✅ Completed
**Description:** Systematic resolution of all pending Rust lints, checks, and styling mistakes across the ewe_platform codebase.

---

### [02: Build HTTP Client](./02-build-http-client/)
**Status:** ⏳ Pending
**Description:** Create an HTTP 1.1 client using existing simple_http module structures with iterator-based patterns and valtron executors.
**Has Features:** Yes (6 features)

| Feature | Description | Tasks | Dependencies |
|---------|-------------|-------|--------------|
| [tls-verification](./02-build-http-client/features/tls-verification/) | Verify/fix TLS backends | 8 | None |
| [foundation](./02-build-http-client/features/foundation/) | Error types and DNS resolution | 7 | tls-verification |
| [connection](./02-build-http-client/features/connection/) | URL parsing, TCP, TLS | 4 | foundation |
| [request-response](./02-build-http-client/features/request-response/) | Request builder, response types | 4 | connection |
| [task-iterator](./02-build-http-client/features/task-iterator/) | TaskIterator, executors | 8 | request-response |
| [public-api](./02-build-http-client/features/public-api/) | User-facing API, integration | 6 | task-iterator |

**Total Tasks:** 37

---

## Status Dashboard

### Summary
- **Total Specifications:** 2
- **Completed:** 1 (50%)
- **In Progress:** 0 (0%)
- **Pending:** 1 (50%)

### Completed ✅
- 01: Fix Rust Lints, Checks, and Styling

### In Progress 🔄
_None_

### Pending ⏳
- 02: Build HTTP Client (6 features, 37 tasks)

## Specification Guidelines

### For Agents
When working with specifications:
1. **Read main files first**: requirements.md AND tasks.md
2. **Check for features/**: If present, read relevant feature.md and tasks.md
3. **Check for templates/**: Read any templates referenced in requirements
4. **Verify before assuming**: Search the codebase to confirm task status
5. **Update as you go**: Mark tasks complete only when truly done
6. **Keep counts accurate**: Update frontmatter in tasks.md files
7. **Commit regularly**: Follow git workflow rules

### For Users
This dashboard provides:
- **Quick overview**: See all specifications at a glance
- **Status tracking**: Monitor progress on each specification
- **Navigation**: Links to detailed requirements and tasks
- **Transparency**: Clear view of what's done, in progress, and pending
- **Feature breakdown**: Understanding of complex specification structure

---
*Last updated: 2026-01-18*
