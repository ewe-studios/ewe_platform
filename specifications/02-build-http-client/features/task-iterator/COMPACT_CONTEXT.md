# Compact Context: Task-Iterator Feature Implementation

⚠️COMPACTED|RELOAD_AFTER_READING|GENERATED:2026-02-01|UPDATED:2026-02-01

# Compact Context: Task-Iterator Feature Implementation

⚠️COMPACTED|RELOAD_AFTER_READING|GENERATED:2026-02-01|UPDATED:2026-02-01v2

## CRITICAL_UPDATES
1.ExecutionAction:apply(&mut self, key, engine) NOT apply(self, executor)
2.execute() returns RecvIterator<TaskStatus> NOT direct Ready value
3.use schedule_iter(Duration) to spawn and get iterator
4.users call single::run_once() or run_until_complete() to drive
5.use ReadyValues::new(iter) to filter for Ready values only
references:valtron/executors/unified.rs (execute),actions.rs (ExecutionAction)

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
        if let Some(data) = self.data.take() {
            spawn_builder(engine).with_parent(key).with_task(task).lift()?;
        }
        Ok(())
    }
}
```

CORRECT execute() wrapper (from valtron/executors/unified.rs):
```rust
fn execute<T>(task: T) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where T: TaskIterator + Send + 'static, ...
{
    let iter = single::spawn().with_task(task).schedule_iter(Duration::from_nanos(5))?;
    Ok(iter)  // Returns iterator, NOT direct Ready value
}

// Usage:
let status_iter = execute(task)?;
single::run_once();  // or run_until_complete()
let ready_values = ReadyValues::new(status_iter);
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
PRIMARY_READ:[backends/foundation_core/src/valtron/executors/unified.rs]
PRIMARY_READ:[backends/foundation_core/src/valtron/executors/actions.rs]
read:[valtron/executors/task.rs,executor.rs,single/mod.rs,multi/mod.rs]

## RETRIEVAL_REQUIRED
PRIMARY:read valtron/executors/unified.rs FIRST|execute() wrapper pattern
PRIMARY:read valtron/executors/actions.rs SECOND|ExecutionAction patterns
verify:execute() returns RecvIterator<TaskStatus>|NOT direct Ready
verify:apply(&mut self, key, engine) signature|Option::take() pattern
check:schedule_iter usage|ReadyValues wrapper|run_once vs run_until_complete
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
1.READ valtron/executors/unified.rs (execute pattern, RecvIterator return, schedule_iter)
2.READ valtron/executors/actions.rs (ExecutionAction apply signature, Option::take)
3.study execute_single,execute_multi impls (schedule_iter usage, Duration params)
4.study test examples (run_once vs run_until_complete, ReadyValues wrapper)
5.write test for RedirectAction (TDD)
6.implement RedirectAction following patterns
7.continue TDD cycle

## BLOCKERS
NONE - specification updated with correct patterns from unified.rs and actions.rs

## CONTEXT_REFS
feature:[features/task-iterator/]
machine_prompt:[./machine_prompt.md]
progress:[../../PROGRESS.md]
PRIMARY_REF:[valtron/executors/unified.rs,valtron/executors/actions.rs]
stack:[.agents/stacks/rust.md]

---
⚠️ SPECIFICATION UPDATED v2: Execute wrapper patterns corrected. Read unified.rs FIRST.

