# Compact Context: Task-Iterator Feature Implementation

⚠️COMPACTED|RELOAD_AFTER_READING|GENERATED:2026-02-01|UPDATED:2026-02-01

## CRITICAL_UPDATE
ExecutionAction signature corrected|apply(&mut self, key, engine) NOT apply(self, parent_key, executor)
reference:valtron/executors/actions.rs|SpawnWithBroadcast,SpawnWithSchedule patterns
pattern:Option::take() for idempotent apply|fields as Option<T>

## CURRENT_TASK
task:implement_task_iterator_feature|status:paused_for_update|started:2026-02-01|tasks:0/11

## MACHINE_PROMPT_CONTENT
feature:task-iterator|priority:high|CRITICAL PATH|unlocks:4 features

EXECUTION_ACTION_SIGNATURE (CORRECTED):
apply(&mut self, key:Entry, engine:BoxedExecutionEngine)->GenericResult<()>
NOT self|NOT executor|use &mut self + engine|idempotent via Option::take()

types:RedirectAction,TlsUpgradeAction,HttpClientAction,HttpRequestTask,HttpRequestState
all_internal:pub(crate)|users never see TaskIterator details

RedirectAction:fields[request:Option<PreparedRequest>,resolver,remaining_redirects,response_sender:Option]
apply(&mut self,key,engine)|if let Some(request)=self.request.take()
spawn_builder(engine).with_parent(key).with_task(redirect_task).lift()

TlsUpgradeAction:fields[connection:Option<Connection>,sni,on_complete:Option]
apply(&mut self,key,engine)|if let Some(conn)=self.connection.take()
spawn_builder(engine).with_parent(key).with_task(tls_task).lift()

HttpClientAction:enum[None,Redirect,TlsUpgrade]|delegates to inner actions
apply(&mut self,key,engine)|match self pattern

spawn_methods:lift() for priority|schedule() for normal|broadcast() for global queue

files:actions.rs,task.rs,executor.rs (NEW)|mod.rs (UPDATE)

## OBJECTIVE
Implement internal TaskIterator machinery with CORRECTED ExecutionAction signatures

## CRITICAL_PATTERN
CORRECT ExecutionAction impl (from valtron/executors/actions.rs):
```rust
impl ExecutionAction for MyAction {
    fn apply(&mut self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        if let Some(data) = self.data.take() {  // Option::take() for idempotent
            let task = MyTask::new(data);
            spawn_builder(engine)  // Use 'engine' NOT 'executor'
                .with_parent(key)   // NOT key.clone()
                .with_task(task)
                .lift()?;           // Or .schedule() or .broadcast()
        }
        Ok(())
    }
}
```

## DEPENDENCIES_MET
valtron-utilities ✅:ExecutionAction types,spawn patterns
foundation ✅:HttpClientError
connection ✅:HttpClientConnection,ParsedUrl
request-response ✅:PreparedRequest,ResponseIntro

## TASKS
1.create actions.rs|RedirectAction with &mut self,engine,Option fields
2.impl TlsUpgradeAction|&mut self,engine,Option::take() pattern
3.create HttpClientAction enum|&mut self for apply
4.impl ExecutionAction for HttpClientAction|delegate with &mut
5.create task.rs|HttpRequestState enum
6.create HttpRequestTask|generic resolver
7.impl TaskIterator|state machine next()
8.create executor.rs|execute_task wrapper
9.impl execute_single|valtron::single::spawn
10.impl execute_multi|feature-gated valtron::multi::spawn
11.write tests|WHY/WHAT docs

## FILES
create:[backends/foundation_core/src/wire/simple_http/client/actions.rs]
create:[backends/foundation_core/src/wire/simple_http/client/task.rs]
create:[backends/foundation_core/src/wire/simple_http/client/executor.rs]
update:[backends/foundation_core/src/wire/simple_http/client/mod.rs]
PRIMARY_READ:[backends/foundation_core/src/valtron/executors/actions.rs]
read:[valtron/executors/task.rs,executor.rs,single/mod.rs,multi/mod.rs]

## RETRIEVAL_REQUIRED
PRIMARY:read valtron/executors/actions.rs FIRST|SpawnWithBroadcast,SpawnWithSchedule patterns
verify:apply(&mut self, key, engine) signature|Option::take() pattern
check:spawn_builder(engine) usage|lift() vs schedule() vs broadcast()
patterns:TaskStatus::Spawn usage|DoNext wrapper

## IMPLEMENTATION_NOTES
signature:apply(&mut self, key:Entry, engine:BoxedExecutionEngine)
fields:Option<T> for all data consumed in apply|take() for idempotent
spawn:spawn_builder(engine) NOT spawn_builder(executor)
parent:with_parent(key) NOT with_parent(key.clone())
methods:lift() priority|schedule() normal|broadcast() global
visibility:pub(crate) ALL types|INTERNAL only

## CONSTRAINTS
package:foundation_core only
NO_ASYNC:no async/await|no tokio|valtron executors only
signature:MUST match valtron/executors/actions.rs pattern
test_docs:MANDATORY WHY/WHAT on every test
simplicity:max 2-3 nesting|clear code

## NEXT_ACTIONS
1.READ valtron/executors/actions.rs (PRIMARY REFERENCE for correct patterns)
2.study SpawnWithBroadcast impl (apply signature, Option::take, spawn_builder usage)
3.study SpawnWithSchedule impl (verify patterns)
4.write test for RedirectAction (TDD)
5.implement RedirectAction following EXACT pattern from actions.rs
6.continue TDD cycle

## BLOCKERS
NONE - specification updated with correct patterns

## CONTEXT_REFS
feature:[features/task-iterator/]
machine_prompt:[./machine_prompt.md]
progress:[../../PROGRESS.md]
PRIMARY_REF:[valtron/executors/actions.rs]
stack:[.agents/stacks/rust.md]

---
⚠️ SPECIFICATION UPDATED: ExecutionAction patterns corrected. Read actions.rs FIRST.

