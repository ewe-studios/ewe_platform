# Compact Context: Implement public-api Feature

⚠️COMPACTED|RELOAD_AFTER_READING|GENERATED:2026-02-02|FROM:[machine_prompt.md,feature.md,rules]

## LOCATION
workspace:[ewe_platform]|spec:[02-build-http-client]|feature:[public-api]|num:[9]
this:[specifications/02-build-http-client/features/public-api/feature.md]
cwd:[bash pwd]|verify:[test -f .agents/AGENTS.md && echo ✓ || echo ✗]

## RULES_SUMMARY
rule:01|naming_structure|ref:[.agents/rules/01-*.md]
rule:02|dir_policy|ref:[.agents/rules/02-*.md]
rule:03|danger_ops|safe_patterns:[test-before-commit]|forbidden:[force-push,destructive-git]|ref:[.agents/rules/03-*.md]
rule:04|commit|verify_first|no_force_push|ref:[.agents/rules/04-*.md]
rule:13|impl_agent|tdd_mandatory|retrieval_first|test_docs_why_what|self_review|ref:[.agents/rules/13-*.md]
rule:14|machine_prompts|58%_reduction|read_machine_not_human|ref:[.agents/rules/14-*.md]
rule:15|context_compact|97%_reduction|reload_after_updates|ref:[.agents/rules/15-*.md]
stack:[rust]|patterns:[pub_everything,Result<T>,trait_bounds,no_unsafe,WHY/WHAT/HOW_docs]|ref:[.agents/stacks/rust.md]
rust-clean-impl:[WHY/WHAT/HOW,derive_more,#[must_use],naming_RFC430]|ref:[.agents/skills/rust-clean-implementation/]
rust-testing:[3_validations,feature_gated,no_muted_vars]|ref:[.agents/skills/rust-testing-excellence/]

## CURRENT_TASK
task:implement_public_api|status:starting|started:2026-02-02

## MACHINE_PROMPT_CONTENT
spec:http-client|status:in-progress|priority:high|feature:public-api|num:9
total_features:13|completed:7|remaining:6|completion:54%

FEATURE:public-api|tasks:17|depends:[foundation✅,connection✅,request-response✅,task-iterator✅]
desc:User-facing API (ClientRequest,SimpleHttpClient)|optional conn pooling|module integration

REQUIREMENTS:
req1:Hide TaskIterator complexity|user sees clean API|SimpleHttpClient::new().get(url).send()
req2:ClientRequest methods|introduction(),body(),send(),parts()|multiple usage patterns
req3:SimpleHttpClient generic DnsResolver|default:SystemDnsResolver|with_resolver(R)
req4:Optional connection pooling|configurable on/off|pool_enabled,pool_max_connections
req5:Feature flags|multi,ssl-rustls,ssl-openssl,ssl-native-tls|in Cargo.toml
req6:HTTPS end-to-end|TLS must work|use existing netcap infrastructure

TASKS:
[ ]task1:Create api.rs with ClientRequest struct|methods:[introduction,body,send,parts,collect]
[ ]task2:Create client.rs with SimpleHttpClient|generic<R:DnsResolver>|default:SystemDnsResolver
[ ]task3:Impl ClientConfig struct|timeouts,redirects,default_headers,pool settings
[ ]task4:Impl SimpleHttpClient::new() and with_resolver()
[ ]task5:Impl builder methods|connect_timeout,max_redirects,enable_pool,config
[ ]task6:Impl convenience methods|get,post,put,delete,patch,head,options
[ ]task7:Impl request() method|takes ClientRequestBuilder|returns ClientRequest
[ ]task8:Create pool.rs with ConnectionPool (optional)|HashMap<PoolKey,Vec<PooledConnection>>
[ ]task9:Impl ClientRequest::introduction()|executes TaskIterator until intro+headers
[ ]task10:Impl ClientRequest::body()|continues TaskIterator for body
[ ]task11:Impl ClientRequest::send()|full TaskIterator execution|returns SimpleResponse
[ ]task12:Impl ClientRequest::parts()|iterator wrapper driving TaskIterator
[ ]task13:Add pub mod client to simple_http/mod.rs
[ ]task14:Add feature flags to Cargo.toml|multi,ssl-*
[ ]task15:Write tests|unit tests for all public API
[ ]task16:Integration tests|HTTP and HTTPS end-to-end
[ ]task17:Verify all commands pass|fmt,clippy,test,build with features

TECH:
stack:[rust]|location:[backends/foundation_core/src/wire/simple_http/client/]
package:foundation_core|avoid:foundation_wasm
existing_types:[ClientRequestBuilder,ResponseIntro,SimpleResponse,SimpleBody,SimpleHeaders,IncomingResponseParts,HttpRequestTask,execute_task]
patterns:[TaskIterator,ExecutionAction,ReadyValues,generic_types]
visibility:[pub_everything per project policy]

VERIFICATION:
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core
cargo build --package foundation_core
cargo build --package foundation_core --features multi
cargo build --package foundation_core --features ssl-rustls
cargo build --package foundation_core --all-features

DEPENDENCIES_VERIFIED:
✅ valtron-utilities (ExecutionAction,unified executor)
✅ tls-verification (rustls,openssl,native-tls backends)
✅ foundation (errors:HttpClientError,DnsError|dns:CachingDnsResolver,SystemDnsResolver)
✅ connection (HttpClientConnection,ParsedUrl,Scheme,TCP,TLS upgrade)
✅ request-response (ClientRequestBuilder,ResponseIntro,PreparedRequest)
✅ task-iterator (HttpRequestTask,execute_task,ReadyValues)

## OBJECTIVE
Implement user-facing public API (SimpleHttpClient,ClientRequest) hiding TaskIterator complexity

## FILES
read:[
  client/mod.rs - module structure
  client/task.rs - HttpRequestTask pattern
  client/executor.rs - execute_task function
  client/request.rs - ClientRequestBuilder
  client/intro.rs - ResponseIntro
  impls.rs - SimpleResponse,SimpleBody,etc
]
create:[
  client/api.rs - ClientRequest
  client/client.rs - SimpleHttpClient
  client/pool.rs - ConnectionPool (optional)
]
update:[
  client/mod.rs - add pub use for new types
  ../mod.rs - already has pub mod client
]

## REQUIREMENTS_REF
feature:[./feature.md]|machine_prompt:[./machine_prompt.md]|spec:[../requirements.md]

## KEY_CONSTRAINTS
1. Hide ALL TaskIterator complexity from users
2. Public API: all types pub (project policy)
3. Generic DnsResolver parameter (no boxing)
4. Reuse existing types (SimpleResponse,ResponseIntro,etc)
5. Connection pooling optional and configurable
6. HTTPS must work end-to-end
7. TDD mandatory - tests first

## RETRIEVAL_CHECKLIST
✅ Read existing client module structure (client/mod.rs)
✅ Understand HttpRequestTask pattern (client/task.rs)
✅ Understand execute_task function (client/executor.rs)
✅ Check existing types to reuse (impls.rs,client/request.rs,client/intro.rs)
[ ] Search for similar API patterns in codebase
[ ] Read ClientRequestBuilder implementation
[ ] Read ResponseIntro implementation
[ ] Check valtron ReadyValues usage

## BLOCKERS
NONE

## NEXT_ACTIONS
1. Generate COMPACT_CONTEXT.md (DONE)
2. Clear context and reload from COMPACT_CONTEXT.md
3. Search for patterns: Grep for ClientRequestBuilder, SimpleResponse usage
4. Read ClientRequestBuilder implementation details
5. Read ResponseIntro and SimpleResponse structures
6. Design ClientRequest API to wrap TaskIterator
7. Implement TDD: Write tests → Verify fail → Implement → Pass
8. Update PROGRESS.md after each major step
9. Regenerate COMPACT_CONTEXT.md after PROGRESS.md updates

## CONTEXT_REFS
progress:[./PROGRESS.md if exists]|learnings:[../LEARNINGS.md]|spec:[../requirements.md]

---
⚠️ AFTER READING THIS FILE: Clear context, reload from this file, proceed with fresh context
