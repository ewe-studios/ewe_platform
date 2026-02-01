# Machine-Optimized Prompt: Task-Iterator Feature

⚠️GENERATED|DO_NOT_EDIT|REGENERATE_FROM:feature.md|GENERATED:2026-02-01|UPDATED:2026-02-01

## META
feature:task-iterator|status:pending|priority:high|effort:medium
depends:[valtron-utilities ✅,foundation ✅,connection ✅,request-response ✅]
tasks:0/11|completion:0%|created:2026-01-18|updated:2026-02-01

## CRITICAL_EXECUTION_ACTION_PATTERN
signature:fn apply(&mut self, key:Entry, engine:BoxedExecutionEngine)->GenericResult<()>
NOT self (use &mut self)|NOT executor (use engine)|idempotent via Option::take()
fields:use Option<T> for take() pattern|example:request:Option<PreparedRequest>
spawn:spawn_builder(engine).with_parent(key).with_task(task).lift/schedule/broadcast()
reference:valtron/executors/actions.rs|SpawnWithBroadcast,SpawnWithSchedule patterns

## SPAWN_METHODS
lift():priority spawn for important tasks|redirects,TLS upgrades
schedule():normal spawn for regular tasks|local queue
broadcast():global queue spawn|cross-thread tasks|requires Send

## OVERVIEW
Internal TaskIterator impl for HTTP requests|ExecutionAction spawners|feature-gated executor wrapper
CRITICAL:all types INTERNAL (pub crate)|users never see TaskIterator/TaskStatus/executor details

## DEPENDENCIES
requires:valtron-utilities ✅,foundation ✅,connection ✅,request-response ✅
used_by:public-api|unlocks:4 features (public-api,cookie-jar,middleware,websocket)
valtron_types:TaskIterator,TaskStatus,ExecutionAction,NoSpawner,DoNext,spawn()

## TYPES_TO_CREATE
RedirectAction:fields[request:Option<PreparedRequest>,resolver,remaining_redirects,response_sender:Option]|apply(&mut self,key,engine)
TlsUpgradeAction:fields[connection:Option<Connection>,sni,on_complete:Option]|apply(&mut self,key,engine)
HttpClientAction:enum[None,Redirect,TlsUpgrade]|apply(&mut self,key,engine) delegates
HttpRequestTask:TaskIterator impl|state machine|fields:[state,resolver,request,remaining_redirects,redirect_receiver]
HttpRequestState:enum[Init,Connecting,TlsHandshake,SendingRequest,ReceivingIntro,ReceivingHeaders,ReceivingBody,AwaitingRedirect,Done,Error]

## EXECUTOR_WRAPPER
execute<T:TaskIterator>()->GenericResult<RecvIterator<TaskStatus<T::Ready,T::Pending,T::Spawner>>>
returns:RecvIterator NOT direct Ready value|users drive with run_once/run_until_complete
wasm:always single|native no multi:single|native with multi:multi
execute_single:single::spawn().with_task(task).schedule_iter(Duration::from_nanos(5))
execute_multi:multi::spawn().with_task(task).schedule_iter(Duration::from_nanos(1))|feature-gated
usage:ReadyValues::new(iter) to filter Ready|single::run_once() for manual|run_until_complete() for auto

## VALTRON_INTEGRATION
TaskIterator:type Pending,Ready,Spawner|fn next()->Option<TaskStatus>
TaskStatus:Ready(T)|Pending(P)|Delayed(P,Duration)|Spawn(P,S)
ExecutionAction:trait|fn apply(&mut self,Entry,BoxedExecutionEngine)->Result
spawn_builder:with_parent(),with_task(),lift()|schedule()|broadcast()

## FILE_STRUCTURE
client/actions.rs:RedirectAction,TlsUpgradeAction,HttpClientAction|NEW
client/task.rs:HttpRequestTask,HttpRequestState|NEW
client/executor.rs:execute_task,execute_single,execute_multi|NEW
client/mod.rs:re-exports|UPDATE

## IMPLEMENTATION_NOTES
visibility:all types pub(crate) or private|INTERNAL ONLY
signature:apply(&mut self, key, engine) NOT apply(self, parent_key, executor)
idempotent:use Option::take() pattern|allows multiple apply calls
state_machine:non-blocking|no loops|use TaskStatus::Spawn for redirects
generic:DnsResolver generic param|Send + 'static bounds
feature_gates:cfg(feature = "multi")|cfg(target_arch = "wasm32")
redirects:spawn via TaskStatus::Spawn(state,RedirectAction)|not blocking
tls:spawn via TaskStatus::Spawn(state,TlsUpgradeAction)|not blocking

## TASKS
[ ]task1:create actions.rs|RedirectAction impl ExecutionAction with &mut self
[ ]task2:impl TlsUpgradeAction|ExecutionAction with &mut self,engine param
[ ]task3:create HttpClientAction enum|combine actions with &mut self
[ ]task4:impl ExecutionAction for HttpClientAction|delegate with correct signature
[ ]task5:create task.rs|HttpRequestState enum
[ ]task6:create HttpRequestTask struct|with generic resolver
[ ]task7:impl TaskIterator for HttpRequestTask|state machine next()
[ ]task8:create executor.rs|execute_task wrapper
[ ]task9:impl execute_single|valtron::single::spawn()
[ ]task10:impl execute_multi|valtron::multi::spawn() feature-gated
[ ]task11:write tests|comprehensive unit tests

## VERIFICATION
cmds:[cargo fmt --check,cargo clippy -D warnings,cargo test --package foundation_core,cargo build --features multi]
tests:actions + task + executor tests|unit coverage
standards:.agents/stacks/rust.md

## SUCCESS_CRITERIA
RedirectAction spawns with &mut self pattern|TlsUpgradeAction uses engine param
HttpClientAction delegates correctly|HttpRequestTask state machine works
execute_task selects correct executor|WASM uses single|multi feature works
tests pass|fmt pass|clippy pass|build with multi feature

## RETRIEVAL_REQUIRED
read:[valtron/executors/unified.rs,valtron/executors/actions.rs] PRIMARY REFERENCES
read:[valtron/executors/task.rs,executor.rs,single/mod.rs,multi/mod.rs]
check:[SpawnWithBroadcast,SpawnWithSchedule impls]|verify:[&mut self,engine,Option::take]
check:[execute() return type,schedule_iter usage,ReadyValues wrapper]
patterns:[Option::take() for idempotent,spawn_builder usage,lift vs schedule]

## DOCS_TO_READ
../requirements.md|./feature.md|valtron/executors/unified.rs (PRIMARY)|valtron/executors/actions.rs (PRIMARY)|request.rs|connection.rs|errors.rs

## CRITICAL_CONSTRAINTS
NO async/await|NO tokio|use valtron executors only
signature:&mut self NOT self|engine NOT executor|Option::take() for idempotent
types:generic not boxed|Send + 'static bounds for multi-threading
visibility:pub(crate) INTERNAL types|users never see these
state_machine:non-blocking iterator pattern|spawn children via TaskStatus::Spawn

---
Token reduction: feature.md (~12KB) → machine_prompt.md (~3KB) = 75% savings
CRITICAL UPDATES: ExecutionAction apply(&mut self,engine), execute() returns RecvIterator
PRIMARY REFS: valtron/executors/unified.rs (execute pattern), actions.rs (ExecutionAction pattern)
