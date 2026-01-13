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

### [01: Fix Rust Lints, Checks, and Styling](./01-fix-rust-lints-checks-styling/)
**Status:** ⏳ Pending
**Description:** Systematic resolution of all pending Rust lints, checks, and styling mistakes across the entire ewe_platform codebase to achieve zero warnings and full compliance with Rust coding standards.

## Status Dashboard

### Summary
- **Total Specifications:** 1
- **Completed:** 0 (0%)
- **In Progress:** 0 (0%)
- **Pending:** 1 (100%)

### Completed ✅
_None yet_

### In Progress 🔄
_None yet_

### Pending ⏳
- 01: Fix Rust Lints, Checks, and Styling

## Specification Guidelines

### For Agents
When working with specifications:
1. **Always read both files**: requirements.md AND tasks.md
2. **Verify before assuming**: Search the codebase to confirm task status
3. **Update as you go**: Mark tasks complete only when truly done
4. **Add new tasks**: Document new work BEFORE starting it
5. **Keep counts accurate**: Update frontmatter in tasks.md
6. **Commit regularly**: Follow git workflow rules

### For Users
This dashboard provides:
- **Quick overview**: See all specifications at a glance
- **Status tracking**: Monitor progress on each specification
- **Navigation**: Links to detailed requirements and tasks
- **Transparency**: Clear view of what's done, in progress, and pending

---
*Last updated: 2026-01-14*
