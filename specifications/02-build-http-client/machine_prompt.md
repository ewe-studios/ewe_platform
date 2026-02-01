# Machine-Optimized Prompt: HTTP 1.1 Client

⚠️GENERATED|DO_NOT_EDIT|REGENERATE_FROM:requirements.md|GENERATED:2026-02-01

## META
spec:http-client|status:in-progress|priority:high|has_features:true|has_fundamentals:true|version:4.0|created:2026-01-18|updated:2026-01-25

## DOCS_TO_READ
requirements.md|LEARNINGS.md|PROGRESS.md|.agents/stacks/rust.md|documentation/simple_http/doc.md|documentation/valtron/doc.md|documentation/netcap/doc.md

## OVERVIEW
HTTP 1.1 client using simple_http structures|iterator-based patterns via TaskIterator|valtron single/multi executors|no async/await|pluggable DNS|optional pooling|TLS via netcap

## APPROACH
patterns:TaskIterator trait,ExecutionAction enum|executors:valtron single/multi modules in foundation_core|types:generic not boxed|dns:pluggable trait|pool:optional configurable|redirect:configurable|tls:netcap infrastructure
CRITICAL:NO async/await|NO tokio|NO hyper|pure iterator-based with valtron execution

## STRUCTURE
type:feature-based|total_features:13|completed:5|remaining:8|completion:38%
location:wire/simple_http/client/|package:foundation_core|avoid:foundation_wasm

## FEATURES_COMPLETED
0:valtron-utilities|tasks:33/33|status:complete
1:tls-verification|tasks:48/48|status:complete|backends:rustls,openssl,native-tls
2:foundation|tasks:9/9|status:complete|errors:HttpClientError,DnsError|dns:caching
4:connection|tasks:11/11|status:complete|tcp:yes|tls:upgrade|url:parsing
6:request-response|tasks:10/10|status:complete|builder:ClientRequestBuilder|response:ResponseIntro

## FEATURES_READY
3:compression|tasks:14|depends:foundation|desc:gzip,deflate,brotli
5:proxy-support|tasks:13|depends:connection|desc:HTTP,HTTPS,SOCKS5
7:auth-helpers|tasks:13|depends:request-response ✅|desc:Basic,Bearer,Digest
8:task-iterator|tasks:11|depends:request-response ✅,valtron-utilities ✅|desc:TaskIterator,executors|RECOMMENDED NEXT

## FEATURES_BLOCKED
9:public-api|tasks:17|depends:task-iterator|desc:SimpleHttpClient,integration
10:cookie-jar|tasks:17|depends:public-api|desc:automatic cookie handling
11:middleware|tasks:13|depends:public-api|desc:request/response interceptors
12:websocket|tasks:17|depends:connection ✅,public-api|desc:WebSocket client/server

## REQUIREMENTS_SUMMARY
req1:iterator-based patterns|no async/await|TaskIterator trait internally
req2:valtron executors|single module + multi module|feature-gated selection
req3:error handling|custom types|derive_more::From,Debug,Display
req4:generic types|not boxed|flexibility via traits
req5:DNS pluggable|custom resolver trait|multiple implementations
req6:connection pooling|optional|configurable on/off
req7:redirect handling|configurable|max redirects or disable
req8:TLS support|netcap infrastructure|reuse existing code

## TECHNICAL
stack:[rust]|NO async/await|NO tokio|NO hyper|location:[wire/simple_http/client/]|package:foundation_core
dependencies:[derive_more,netcap]|valtron:foundation_core valtron module|patterns:[TaskIterator,ExecutionAction,generic types]
errors:[HttpClientError,DnsError]|dns:caching LRU|tls:rustls/openssl/native-tls
execution:valtron single/multi executors|iterator-based patterns only

## VERIFICATION
per_feature:see features/[name]/feature.md|spec_wide:[cargo clippy,cargo test,cargo build]
quality:zero warnings|zero failures|integration tests|cross-feature tests
standards:.agents/stacks/rust.md|documentation:update after implementation

## SUCCESS_CRITERIA
criteria1:all 13 features complete|marked ✅ in feature index
criteria2:all inter-feature integration tests passing
criteria3:cross-feature functionality verified
criteria4:zero clippy warnings across all features
criteria5:all tests passing|cargo test --package foundation_core
criteria6:fundamentals/ directory with user documentation
criteria7:LEARNINGS.md + REPORT.md + VERIFICATION.md created

## RETRIEVAL_CHECKLIST
search_similar:[simple_http module patterns]|read_existing:[wire/simple_http/*]
check_patterns:[TaskIterator usage,valtron executor patterns]|verify:[netcap TLS integration]
stack_file:[.agents/stacks/rust.md]|module_docs:[documentation/*/doc.md]

## FEATURE_DETAILS
format:each feature has features/[name]/feature.md with detailed requirements
loading:agents load specific feature.md as needed|not all features at once
structure:feature.md contains tasks,dependencies,verification,success criteria
context_optimization:agents read overview + current feature only

## KNOWN_ISSUES
foundation_wasm:~110 compilation errors|status:OUT OF SCOPE|workaround:use foundation_core only
workspace:avoid foundation_wasm package|all HTTP client code in foundation_core

## PROGRESS_STATUS
last_completed:request-response feature|date:2026-02-01
ready_to_start:compression,proxy-support,auth-helpers,task-iterator|dependencies met
recommended_next:task-iterator|unlocks:public-api and 3 more features|critical path

## FILES_REQUIRED
main_agent:[.agents/rules/01-06,14-15]|files:[requirements.md,LEARNINGS.md,PROGRESS.md]
verification_agent:[.agents/rules/01-04,08,14-15,.agents/stacks/rust.md]|files:[requirements.md]
implementation_agent:load features/[name]/feature.md per feature|files_required in feature frontmatter

## CONTEXT_OPTIMIZATION
rule14:machine_prompt.md generated|58% token reduction|sub-agents read this not requirements.md
rule15:COMPACT_CONTEXT.md generated per task|97% context reduction|embed machine_prompt content
workflow:generate machine_prompt→clear context→reload→generate compact_context→spawn sub-agent
lifecycle:COMPACT_CONTEXT ephemeral per task|deleted on completion|regenerated on PROGRESS.md update

---
Token reduction: requirements.md (~13KB) → machine_prompt.md (~3KB) = 77% savings
Sub-agents: Read this file instead of verbose requirements.md
Feature agents: Read overview here + specific features/[name]/feature.md
