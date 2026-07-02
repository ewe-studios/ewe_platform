# Specification 52: Tauri Foundation Platform

## Overview
This specification defines the creation of a new `foundation_platform` crate to manage cross-platform (desktop, mobile, web) capabilities using Tauri. It will integrate with the existing WASM UI foundation established in `specifications/completed/39-foundation-wasm-ui`.

## Goals
- Create `foundation_platform` crate.
- Integrate Tauri for cross-platform support.
- Leverage learnings from Basecamp's Hotwire Native approach for server-rendered app integration.
- Ensure seamless integration with WASM UI.

## Decisions
- All architectural and implementation decisions will be documented in `decisions/`.

## Plan
1. Outline initial decisions in `decisions/01-platform-architecture.md`.
2. Create `PLAN.md` to ground explorations.
3. Explore Basecamp Hotwire Native sources.
4. Implement `foundation_platform` crate.
5. Integrate with WASM UI.
