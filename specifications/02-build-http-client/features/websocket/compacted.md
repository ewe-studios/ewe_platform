# Compacted Context: WebSocket Feature Implementation

⚠️TEMP_FILE|DELETE_WHEN_DONE|GENERATED:2026-03-10

## LOCATION
workspace:ewe_platform|spec:02-build-http-client|file:specifications/02-build-http-client/features/websocket/

## OBJECTIVE
Implement WebSocketTask (TaskIterator) and WebSocketConnection for RFC 6455 client support

## REQUIREMENTS
req:TaskIterator pattern|no async/await|sync only|one step per next()
req:Executor boundary via unified::execute_stream()|NOT raw TaskIterator wrapping
req:RFC 6455 compliance|masking|fragmentation|control frames|close handshake
req:TDD|one test at a time|#[traced_test] attribute

## TASKS (from feature.md Phase 1)
[x]1-frame.rs|done with tests
[x]2-handshake.rs|done with tests
[x]3-message.rs|done (basic enum)
[x]4-error.rs|done
[ ]5-task.rs|WebSocketTask TaskIterator - NEXT
[ ]6-connection.rs|WebSocketConnection + WebSocketClient wrapper
[ ]7-integration tests

## LEARNINGS (from LEARNINGS.md)
past1:TaskIterator=state machine|Option<State> wrapper|self.state.take()? pattern
past2:NO loops in next()|ONE transition per call
past3:Executor boundary via unified::execute_stream()|returns DrivenStreamIterator
past4:Consumer wraps DrivenStreamIterator|NOT TaskIterator directly
past5:Connection failures→intermediate state|tests can observe failure progression
past6:use HttpConnectionPool::create_http_connection()|not manual DNS+Connection

## CURRENT_STATE
progress:frame+handshake+message+error done|task.rs missing|connection.rs missing
next:implement WebSocketTask in task.rs
blockers:NONE

## FILES_TO_MODIFY
read:backends/foundation_core/src/wire/websocket/|valtron/task.rs|valtron/executors/unified.rs|wire/event_source/task.rs|wire/event_source/consumer.rs
create:backends/foundation_core/src/wire/websocket/task.rs
create:backends/foundation_core/src/wire/websocket/connection.rs

## NEXT_ACTIONS
1. Read valtron/task.rs and event_source/task.rs for TaskIterator patterns
2. Create task.rs with WebSocketTask state machine
3. Implement Init→Connecting→Handshake→Open→Closed states
4. Create connection.rs with WebSocketConnection blocking API
5. Create WebSocketClient consumer wrapper using execute_stream()

## DEPENDENCIES NEEDED
Cargo.toml:sha1=0.10|rand=0.8|base64=0.21 (check if already in foundation_core)

---

⚠️AFTER READING:Clear context,reload from this file only,start work
