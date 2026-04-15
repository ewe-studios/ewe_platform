# Compacted Context: foundation_errstacks Implementation (Phase 1 Tasks 1.1-1.3)

TEMP_FILE|DELETE_WHEN_DONE|GENERATED:2026-04-15

## LOCATION
workspace:ewe_platform|spec:08-foundation-errstacks|target:backends/foundation_errstacks

## OBJECTIVE
Bootstrap foundation_errstacks crate and implement ErrorTrace+Frame core types via TDD (tasks 1.1-1.3 only, then STOP).

## STACK
lang:rust|msrv:1.81|workspace_rust:1.87|edition:2021|tests_dir:tests/|no_cfg_test_in_src

## REQUIREMENTS (§3-4 requirements.md)
arch:ErrorTrace<C>{frames:Box<Vec<Frame>>,PhantomData<fn()->*const C>}
frame:Frame{frame:Box<dyn FrameImpl>,sources:Box<[Frame]>}
frameimpl:trait{kind()->FrameKind<'_>,as_any,as_any_mut}|Send+Sync+'static
framekind:Context(&dyn core::error::Error)|Attachment(Printable|Opaque)
attachment:Printable(&dyn Display+Debug)|Opaque(&dyn Any)
methods:new,change_context,attach,attach_opaque,attach_with,attach_opaque_with,downcast_ref,contains,frames,current_context
cargo:derive_more={version="2",default-features=false,features=["display","error","from"]}
features:default=["std"]|alloc=[]|std=["alloc"]|serde=["alloc","dep:serde"]|backtrace=["std"]|async=["std","dep:futures-core"]|slack=["serde","dep:serde_json"]
lib_rs:#![cfg_attr(not(feature="std"),no_std)]|extern crate alloc;|use core::error::Error
pitfalls:NEVER loop{} in Iterator::next|use #[track_caller]|tests in tests/ not #[cfg(test)] in src
deps_order:project→stdlib→external|logging MUST use tracing crate

## TASKS (this run ONLY 1.1-1.3)
[ ]1.1:Create crate skeleton backends/foundation_errstacks/{Cargo.toml,src/lib.rs,tests/mod.rs}|features per §3.5.2|workspace member auto via backends/*
[ ]1.2:Impl ErrorTrace<C> struct+ErrorTrace::new()+change_context|write test FIRST in tests/
[ ]1.3:Impl Frame+FrameImpl+FrameIter+FrameKind+AttachmentKind|test FIRST
STOP after 1.3|report to main|update PROGRESS.md|do NOT commit

## WORKSPACE CONVENTIONS (observed)
- backends/* auto-included as workspace members
- Cargo.toml uses edition.workspace/rust-version.workspace/etc.
- tests/mod.rs = single integration binary importing sub test modules
- foundation_nostd pattern: `#![cfg_attr(not(feature="std"),no_std)] #[cfg(not(feature="std"))] extern crate alloc;` — but spec wants unconditional `extern crate alloc;` because alloc is baseline
- workspace derive_more version >=2.0 with features=["full"] (but we override with default-features=false per spec)

## TDD CYCLE
write ONE test→verify fails→implement→verify passes→next

## NEXT_ACTIONS
1. Create Cargo.toml (foundation_errstacks, features per spec, derive_more no-default)
2. Create src/lib.rs skeleton with no_std attrs + extern crate alloc + module decls
3. Task 1.2: write tests/units/errstacks_error_trace_tests.rs test_new_creates_trace → run → fail → implement ErrorTrace::new → pass
4. Task 1.2b: test change_context → fail → impl → pass
5. Task 1.3: test frame iteration / FrameKind → fail → impl Frame/FrameImpl/FrameIter → pass
6. Update PROGRESS.md checkmarks for 1.1, 1.2, 1.3
7. Report & STOP
